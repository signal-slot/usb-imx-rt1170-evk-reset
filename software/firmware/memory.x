MEMORY {
    BOOT2 : ORIGIN = 0x10000000, LENGTH = 0x100
    FLASH : ORIGIN = 0x10000100, LENGTH = 2048K - 0x100
    RAM   : ORIGIN = 0x20000000, LENGTH = 264K
}

/* Force the BSP's second-stage bootloader symbol to be linked even though
 * nothing references it from Rust. */
EXTERN(BOOT2_FIRMWARE)

SECTIONS {
    /* The RP2040 mask ROM jumps to flash offset 0; the first 256 bytes must
     * be the boot2 stage that configures XIP for our SPI flash chip. */
    .boot2 ORIGIN(BOOT2) :
    {
        KEEP(*(.boot2));
    } > BOOT2
} INSERT BEFORE .text;
