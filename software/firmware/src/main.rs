#![no_std]
#![no_main]

use panic_halt as _;

use seeeduino_xiao_rp2040 as bsp;

use bsp::{
    entry,
    hal::{
        clocks::init_clocks_and_plls,
        gpio::FunctionSioOutput,
        pac,
        sio::Sio,
        timer::Timer,
        usb::UsbBus,
        watchdog::Watchdog,
    },
    Pins, XOSC_CRYSTAL_FREQ,
};

use embedded_hal::digital::OutputPin;

use usb_device::{
    class_prelude::UsbBusAllocator,
    device::{StringDescriptors, UsbDevice, UsbDeviceBuilder, UsbVidPid},
};
use usbd_serial::{SerialPort, USB_CLASS_CDC};

// `seeeduino-xiao-rp2040` の `boot2` feature がデフォルトで
// `BOOT2_FIRMWARE` を `.boot2` セクションに配置してくれるので、
// ここで重ねて宣言する必要はない。

const DEFAULT_RESET_MS: u32 = 100;
const MIN_RESET_MS: u32 = 10;
const MAX_RESET_MS: u32 = 1000;

#[entry]
fn main() -> ! {
    let mut pac = pac::Peripherals::take().unwrap();

    let mut watchdog = Watchdog::new(pac.WATCHDOG);

    let clocks = init_clocks_and_plls(
        XOSC_CRYSTAL_FREQ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    let sio = Sio::new(pac.SIO);

    let pins = Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    /*
     * XIAO RP2040:
     *   シルクスクリーン D7 = GPIO1
     *   ※ seeeduino-xiao-rp2040 BSP では D7 のフィールド名は `rx`
     *     (デフォルト機能が UART0 RX のため)
     *
     * 配線:
     *   D7/GPIO1 -> 470Ω -> TLP241A pin1
     *   GND      --------> TLP241A pin2
     *
     * GPIO1 = High → フォトリレー ON  (EVKB RESET 印加)
     * GPIO1 = Low  → フォトリレー OFF (EVKB RESET 解除)
     */
    let mut reset_pin = pins.rx.into_function::<FunctionSioOutput>();

    // 起動直後は必ず OFF。USB ホストの列挙中に EVKB をリセットしないため。
    reset_pin.set_low().ok();

    let timer = Timer::new(pac.TIMER, &mut pac.RESETS, &clocks);

    let usb_bus = UsbBusAllocator::new(UsbBus::new(
        pac.USBCTRL_REGS,
        pac.USBCTRL_DPRAM,
        clocks.usb_clock,
        true,
        &mut pac.RESETS,
    ));

    let mut serial = SerialPort::new(&usb_bus);

    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x1209, 0x0001))
        .strings(&[StringDescriptors::default()
            .manufacturer("Signal Slot")
            .product("EVKB USB Reset")
            .serial_number("XIAO-RP2040-RESET-001")])
        .unwrap()
        .device_class(USB_CLASS_CDC)
        .build();

    let mut line_buf = LineBuffer::new();

    loop {
        if usb_dev.poll(&mut [&mut serial]) {
            let mut buf = [0u8; 64];

            if let Ok(n) = serial.read(&mut buf) {
                for &b in &buf[..n] {
                    if let Some(line) = line_buf.push(b) {
                        match handle_line(line, &mut serial) {
                            Action::None => {}
                            Action::SetLow => {
                                reset_pin.set_low().ok();
                                write_all(&mut serial, b"OK OFF\r\n");
                            }
                            Action::SetHigh => {
                                reset_pin.set_high().ok();
                                write_all(&mut serial, b"OK ON\r\n");
                            }
                            Action::Pulse(ms) => {
                                pulse_high(
                                    &mut reset_pin,
                                    &timer,
                                    ms,
                                    &mut usb_dev,
                                    &mut serial,
                                );
                                write_all(&mut serial, b"OK RESET ");
                                write_u32(&mut serial, ms);
                                write_all(&mut serial, b"\r\n");
                                // pulse 中に来た受信ゴミで LineBuffer が
                                // 中途状態のままになっていることがあるので
                                // 念のため空に戻す。
                                line_buf.reset();
                            }
                            Action::Bootsel => {
                                write_all(&mut serial, b"OK BOOTSEL\r\n");
                                // 応答を IN エンドポイントから流し切るまで
                                // 50 ms ほど USB を poll してから ROM に飛ぶ。
                                let start_us = timer.get_counter().ticks();
                                while timer.get_counter().ticks() - start_us
                                    < 50_000
                                {
                                    let _ = usb_dev.poll(&mut [&mut serial]);
                                }
                                bsp::hal::rom_data::reset_to_usb_boot(0, 0);
                                // ROM 呼び出しはチップを reset するので
                                // 戻ってこない。万一戻ってもここで停止。
                                loop {
                                    core::hint::spin_loop();
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

enum Action {
    None,
    SetLow,
    SetHigh,
    Pulse(u32),
    Bootsel,
}

fn handle_line<B>(line: &str, serial: &mut SerialPort<'_, B>) -> Action
where
    B: usb_device::bus::UsbBus,
{
    let line = line.trim();

    if line.eq_ignore_ascii_case("PING") {
        write_all(serial, b"PONG\r\n");
        return Action::None;
    }

    if line.eq_ignore_ascii_case("OFF") {
        return Action::SetLow;
    }

    if line.eq_ignore_ascii_case("ON") {
        return Action::SetHigh;
    }

    if line.eq_ignore_ascii_case("BOOTSEL") {
        return Action::Bootsel;
    }

    if let Some(ms) = parse_reset_command(line) {
        return Action::Pulse(ms.clamp(MIN_RESET_MS, MAX_RESET_MS));
    }

    write_all(serial, b"ERR\r\n");
    Action::None
}

fn parse_reset_command(line: &str) -> Option<u32> {
    let mut parts = line.split_whitespace();

    let cmd = parts.next()?;

    if !cmd.eq_ignore_ascii_case("RESET") {
        return None;
    }

    match parts.next() {
        Some(ms) => ms.parse::<u32>().ok(),
        None => Some(DEFAULT_RESET_MS),
    }
}

/// HIGH を `ms` ミリ秒だけ駆動して LOW に戻す。
///
/// delay 中も USB を poll し続けるのが要点。さもないと:
///   - ホスト → デバイスへの CDC OUT データがエンドポイント FIFO に滞留し、
///     pulse 完了後にまとめて読み出されて「謎の ERR 連発」になる。
///   - デバイス → ホストへの IN にも応答が遅れて、ホストの再送/再列挙を
///     誘発することがある。
/// pulse 中に受信したバイトは LineBuffer には流さず破棄する。pulse 中に
/// 別コマンドを受け付けてしまうとリセット線の挙動が予測不能になるため。
fn pulse_high<P, B>(
    reset_pin: &mut P,
    timer: &Timer,
    ms: u32,
    usb_dev: &mut UsbDevice<'_, B>,
    serial: &mut SerialPort<'_, B>,
) where
    P: OutputPin,
    B: usb_device::bus::UsbBus,
{
    reset_pin.set_high().ok();

    let start_us = timer.get_counter().ticks();
    let target_us = start_us + ms as u64 * 1000;

    while timer.get_counter().ticks() < target_us {
        if usb_dev.poll(&mut [serial]) {
            let mut sink = [0u8; 64];
            let _ = serial.read(&mut sink);
        }
    }

    reset_pin.set_low().ok();
}

fn write_all<B: usb_device::bus::UsbBus>(serial: &mut SerialPort<'_, B>, mut bytes: &[u8]) {
    while !bytes.is_empty() {
        match serial.write(bytes) {
            Ok(0) | Err(_) => return,
            Ok(n) => bytes = &bytes[n..],
        }
    }
}

fn write_u32<B: usb_device::bus::UsbBus>(serial: &mut SerialPort<'_, B>, mut v: u32) {
    let mut buf = [0u8; 10];
    let mut i = buf.len();

    loop {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }

    write_all(serial, &buf[i..]);
}

struct LineBuffer {
    buf: [u8; 64],
    len: usize,
}

impl LineBuffer {
    const fn new() -> Self {
        Self {
            buf: [0; 64],
            len: 0,
        }
    }

    fn reset(&mut self) {
        self.len = 0;
    }

    fn push(&mut self, b: u8) -> Option<&str> {
        match b {
            b'\r' | b'\n' => {
                if self.len == 0 {
                    return None;
                }

                let line = core::str::from_utf8(&self.buf[..self.len]).ok();

                self.len = 0;

                line
            }
            _ => {
                if self.len < self.buf.len() {
                    self.buf[self.len] = b;
                    self.len += 1;
                } else {
                    // 入力が長すぎたら破棄。
                    self.len = 0;
                }

                None
            }
        }
    }
}
