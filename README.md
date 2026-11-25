# 🌊 WaiCheck — Low-Cost Solar River Water Quality Monitor

**Created by Liam Watson & Kevin Lo** | *FIRST LEGO League (FLL) Innovation Project — November 2026*

**WaiCheck** is an inexpensive, solar-powered custom alternative to traditional scientific water quality checkers. Built around an **ESP32 microcontroller** running **bare-metal asynchronous Rust (`no_std`)**, it monitors river pH, turbidity, and water presence in real time and streams telemetry to a public web dashboard.

🔗 **Live Dashboard:** [WaiCheck River Monitor](https://replit.app)

---

## ⚡ Why Rust on an ESP32?
Switching from a Raspberry Pi 5 to an ESP32 saved **\$213 NZD** per unit, bringing total build cost down to **\$49 NZD** (compared to **\$419 NZD** if sourced locally). To achieve high performance within the ESP32's tight RAM and CPU limits, we wrote the firmware in zero-allocation `no_std` Rust using `embassy`:

| Language | Execution Time (40M Ops) | Time Saved | Relative Speed |
| :--- | :---: | :---: | :---: |
| Python | 27.3172 s | Baseline | 1.0× |
| **Rust** | **0.3382 s** | **26.9790 s (98.8% faster)** | **80.8× faster** |

---

## 🛠️ Hardware & Bill of Materials (~49 NZD Total)

### Outside Enclosure
* **Waterproof Outer Shell:** [AliExpress Listing](https://aliexpress.com)
* **Solar Panel (5V–6V DC):** [AliExpress Listing](https://aliexpress.com)

### Inside Electronics
* **ESP32 Board (240 MHz, 3.3V Logic):** [AliExpress Listing](https://aliexpress.com)
* **Solar Charge Board (4.2V Li-ion Controller):** [AliExpress Listing](https://aliexpress.com)
* **Rechargeable Battery (3.7V Li-ion/LiPo):** [AliExpress Listing](https://aliexpress.com)
* **pH Sensor Module (BNC Probe):** [AliExpress Listing](https://aliexpress.com)
* **Turbidity Sensor (Optical):** [AliExpress Listing](https://aliexpress.com)
* **Water Level Sensor:** [AliExpress Listing](https://aliexpress.com)

### Signal Conditioning & Protection
* **Bulk Decoupling Capacitor (`100µF–470µF`):** Prevents brownout resets during Wi-Fi transmission bursts.
* **High-Frequency Bypass Capacitors (`100nF` ceramic):** Filters electrical noise on the pH and turbidity sensor rails.
* **Voltage Divider (`10kΩ` & `20kΩ`):** Steps down 5V analog sensor outputs to `≤ 3.3V` to protect ESP32 ADC1 pins (`GPIO32`, `GPIO34`, `GPIO35`).

---

## 🚀 Building & Flashing the Firmware

1. **Install the Rust ESP32 toolchain:**
   ```bash
   cargo install espup espflash
   espup install
   . $HOME/export-esp.sh
   ```
2. **Configure Wi-Fi credentials** in `src/main.rs` (`WIFI_SSID` and `WIFI_PASSWORD`).
3. **Compile and flash** with Link-Time Optimization enabled:
   ```bash
   cargo run --release
   ```
