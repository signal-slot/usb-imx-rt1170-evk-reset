↑ [【rt1170reset_R05】ハードウェア接続条件](./rt1170reset_R05.md "【rt1170reset_R05】")

# 【rt1170reset_R05.DM01】配線構成

## 要求

USB ホスト、XIAO RP2040、TLP241A、RT1170-EVK J1 の接続を仕様として固定する

### 理由

実装者が別配線を前提にコードを書いてしまうことを防ぐため

### 説明

XIAO RP2040 の PIN7 は TLP241A の入力 LED を 470Ω 経由で駆動する

## 仕様

| 種別 | 内容 | 仕様番号 |
| --- | --- | --- |
| 仕様 | USB ホストは XIAO RP2040 と USB 接続され、CDC シリアル経由でコマンドを送る | 【rt1170reset_R05.DM01-01】 |
| 仕様 | XIAO RP2040 の PIN7 は 470Ω を介して TLP241A の PIN1 に接続される | 【rt1170reset_R05.DM01-02】 |
| 仕様 | XIAO RP2040 の GND は TLP241A の PIN2 に接続される | 【rt1170reset_R05.DM01-03】 |
| 仕様 | TLP241A の PIN3 と PIN4 は RT1170-EVK の J1 20 ピンの 15 番と 16 番に接続される | 【rt1170reset_R05.DM01-04】 |
