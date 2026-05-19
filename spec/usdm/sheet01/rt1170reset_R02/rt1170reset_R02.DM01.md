↑ [【rt1170reset_R02】GPIOパルス制御](./rt1170reset_R02.md "【rt1170reset_R02】")

# 【rt1170reset_R02.DM01】GPIO1によるフォトリレー駆動

## 要求

XIAO RP2040 の PIN7 から TLP241A の入力 LED を駆動し、PIN3 と PIN4 の導通で RT1170-EVK の J1 15 番と 16 番を短絡する

### 理由

マイコン側と EVK 側を絶縁したままリセット操作を実現するため

### 説明

PIN7 のシルク名は D7 であり、`seeeduino-xiao-rp2040` BSP 上では `pins.rx` が GPIO1 出力に対応する

## 仕様

| 種別 | 内容 | 仕様番号 |
| --- | --- | --- |
| 仕様 | XIAO RP2040 の PIN7 を `pins.rx` 経由で GPIO1 のデジタル出力として構成する | 【rt1170reset_R02.DM01-01】 |
| 仕様 | PIN7 が High の間だけ TLP241A の PIN1-PIN2 間 LED を点灯させる | 【rt1170reset_R02.DM01-02】 |
| 仕様 | TLP241A の PIN3 と PIN4 の導通により RT1170-EVK の J1 15 番と 16 番を短絡する | 【rt1170reset_R02.DM01-03】 |
| 仕様 | PIN7 が Low のときは TLP241A の出力を非導通とし、J1 15 番と 16 番を開放する | 【rt1170reset_R02.DM01-04】 |
