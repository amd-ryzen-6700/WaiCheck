#![no_std]
#![no_main]

use core::fmt::Write;
use embassy_executor::Spawner;
use embassy_net::{
    dns::DnsSocket,
    tcp::client::{TcpClient, TcpClientState},
    Config as NetConfig, StackResources,
};
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::{
    analog::adc::{Adc, AdcConfig, Attenuation},
    clock::CpuClock,
    gpio::Io,
    rng::Rng,
    timer::timg::TimerGroup,
};
use esp_wifi::{
    init,
    wifi::{ClientConfiguration, Configuration, WifiController, WifiDevice, WifiEvent, WifiStaDevice, WifiState},
    EspWifiController,
};
use heapless::String;
use log::{error, info};
use reqwless::{
    client::{HttpClient, TlsConfig, TlsVerify},
    headers::ContentType,
    request::Method,
};

const WIFI_SSID: &str = "YOUR_WIFI_SSID";
const WIFI_PASSWORD: &str = "YOUR_WIFI_PASSWORD";
const DASHBOARD_URL: &str = "https://replit.app";

macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(72 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_hal_embassy::init(timg0.timer0);

    let mut rng = Rng::new(peripherals.RNG);
    let timg1 = TimerGroup::new(peripherals.TIMG1);

    let init = &*mk_static!(
        EspWifiController<'static>,
        init(timg1.timer0, rng.clone(), peripherals.RADIO_CLK).unwrap()
    );
    let wifi = peripherals.WIFI;
    let (wifi_interface, controller) =
        esp_wifi::wifi::new_with_mode(init, wifi, WifiStaDevice).unwrap();

    let net_config = NetConfig::dhcpv4(Default::default());
    let seed = (rng.random() as u64) << 32 | rng.random() as u64;

    let (stack, runner) = embassy_net::new(
        wifi_interface,
        net_config,
        mk_static!(StackResources<3>, StackResources::<3>::new()),
        seed,
    );

    spawner.spawn(connection_task(controller)).ok();
    spawner.spawn(net_task(runner)).ok();

    let io = Io::new(peripherals.IO_MUX);
    let mut adc1_config = AdcConfig::new();
    let mut ph_pin = adc1_config.enable_pin(io.pins.gpio34, Attenuation::Attenuation11dB);
    let mut turbidity_pin = adc1_config.enable_pin(io.pins.gpio35, Attenuation::Attenuation11dB);
    let mut water_lvl_pin = adc1_config.enable_pin(io.pins.gpio32, Attenuation::Attenuation11dB);
    let mut adc1 = Adc::new(peripherals.ADC1, adc1_config);

    info!("Waiting for Wi-Fi connection...");
    while !stack.is_link_up() {
        Timer::after(Duration::from_millis(200)).await;
    }
    while stack.config_v4().is_none() {
        Timer::after(Duration::from_millis(200)).await;
    }
    info!("Connected! IP: {:?}", stack.config_v4().unwrap().address);

    let client_state = TcpClientState::<1, 4096, 4096>::new();
    let tcp_client = TcpClient::new(stack, &client_state);
    let dns_socket = DnsSocket::new(stack);
    let mut rx_buf = [0u8; 4096];
    let mut tls_read_buf = [0u8; 16384];
    let mut tls_write_buf = [0u8; 4096];

    loop {
        let raw_ph = read_adc_oversampled(&mut adc1, &mut ph_pin);
        let raw_turb = read_adc_oversampled(&mut adc1, &mut turbidity_pin);
        let raw_water = read_adc_oversampled(&mut adc1, &mut water_lvl_pin);

        let ph_volts = (raw_ph as f32 / 4095.0) * 3.3;
        let ph_value = 7.0 + ((2.5 - ph_volts) / 0.18);

        let turb_volts = ((raw_turb as f32 / 4095.0) * 3.3) * 1.5;
        let turbidity_ntu = (-1120.4 * turb_volts * turb_volts + 5742.3 * turb_volts - 4352.9).max(0.0);

        let water_detected = raw_water > 1000;

        let mut json_payload: String<128> = String::new();
        let _ = write!(
            json_payload,
            r#"{{"ph":{:.2},"turbidity":{:.1},"water_detected":{}}}"#,
            ph_value, turbidity_ntu, water_detected
        );

        info!("Sending payload: {}", json_payload.as_str());

        let tls_config = TlsConfig::new(
            seed,
            &mut tls_read_buf,
            &mut tls_write_buf,
            TlsVerify::None,
        );
        let mut http_client = HttpClient::new_with_tls(&tcp_client, &dns_socket, tls_config);

        match http_client.request(Method::POST, DASHBOARD_URL).await {
            Ok(mut req) => {
                let req = req
                    .body(json_payload.as_bytes())
                    .content_type(ContentType::ApplicationJson);

                match req.send(&mut rx_buf).await {
                    Ok(resp) => info!("Dashboard HTTP Status: {}", resp.status.0),
                    Err(e) => error!("HTTP Send failed: {:?}", e),
                }
            }
            Err(e) => error!("HTTP Request init failed: {:?}", e),
        }

        Timer::after(Duration::from_secs(10)).await;
    }
}

#[inline(always)]
fn read_adc_oversampled<ADCI, PIN>(adc: &mut Adc<'_, ADCI>, pin: &mut PIN) -> u16
where
    ADCI: esp_hal::analog::adc::RegisterAccess,
    PIN: esp_hal::analog::adc::AdcChannel + embedded_hal::adc::nb::Channel<ADCI, ID = u8>,
{
    let mut sum: u32 = 0;
    for _ in 0..16 {
        if let Ok(val) = nb::block!(adc.read_oneshot(pin)) {
            sum += val as u32;
        }
    }
    (sum >> 4) as u16
}

#[embassy_executor::task]
async fn connection_task(mut controller: WifiController<'static>) {
    loop {
        if esp_wifi::wifi::wifi_state() == WifiState::StaConnected {
            controller.wait_for_event(WifiEvent::StaDisconnected).await;
            Timer::after(Duration::from_secs(2)).await;
        }
        if !matches!(controller.is_started(), Ok(true)) {
            let client_config = Configuration::Client(ClientConfiguration {
                ssid: WIFI_SSID.try_into().unwrap(),
                password: WIFI_PASSWORD.try_into().unwrap(),
                ..Default::default()
            });
            controller.set_configuration(&client_config).unwrap();
            controller.start_async().await.unwrap();
        }
        let _ = controller.connect_async().await;
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static, WifiDevice<'static, WifiStaDevice>>) {
    runner.run().await
}
