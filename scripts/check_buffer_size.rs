use cpal::traits::{DeviceTrait, HostTrait};

#[cfg(target_os = "macos")]
fn main() {
    let host = cpal::host_from_id(cpal::HostId::CoreAudio).unwrap();

    println!("Output devices:");
    for device in host.output_devices().unwrap() {
        list_output_device_options(&device);
    }
    println!("\n\n\nInput devices:");
    for device in host.input_devices().unwrap() {
        list_input_device_options(&device);
    }
}

#[cfg(target_os = "windows")]
fn main() {
    let host = cpal::host_from_id(cpal::HostId::Wasapi).unwrap();

    println!("Output devices:");
    for device in host.output_devices().unwrap() {
        list_output_device_options(&device);
    }
    println!("\n\n\nInput devices:");
    for device in host.input_devices().unwrap() {
        list_input_device_options(&device);
    }
}

fn list_output_device_options(device: &cpal::Device) {
    let device_description = device.description().unwrap();
    let name = device_description.name();
    if !name.contains("Voicemeeter") {
        return;
    }
    println!("Device: {}", name);
    let config = device.default_output_config().unwrap();
    show_config(&config);
}

fn list_input_device_options(device: &cpal::Device) {
    let device_description = device.description().unwrap();
    let name = device_description.name();
    if !name.contains("Voicemeeter") {
        return;
    }
    println!("Device: {}", name);
    let config = device.default_input_config().unwrap();
    show_config(&config);
}

fn show_config(config: &cpal::SupportedStreamConfig) {
    println!("      Sample rate: {}", config.sample_rate());
    println!("      Channels: {}", config.channels());
    println!("      Buffer size: {:?}", config.buffer_size());
}
