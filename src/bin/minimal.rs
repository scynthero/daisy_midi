#![no_main]
#![no_std]

use daisy_midi as _; // global logger + panicking-behavior + memory layout


#[rtic::app(
// device = stm32h7xx_hal::stm32,
device = daisy_bsp::hal::pac,
// dispatchers = [TIM2],
peripherals = true,
)]
mod app {
    // Add a monotonic if scheduling will be used
    // #[monotonic(binds = SysTick, default = true)]
    // type DwtMono = DwtSystick<80_000_000>;
    use daisy_bsp::hal::{
        usb_hs::{UsbBus, USB2},
        rcc::rec::UsbClkSel};
    use daisy_bsp::led::UserLed;
    use daisy_bsp::hal::prelude::*;
    use daisy_bsp::led::Led;
    use usb_device::prelude::*;
    use usbd_midi::{
        data::{
            byte::{
                u7::U7
            },
            midi::{
                message::Message,
            },
            usb_midi::{
                midi_packet_reader::MidiPacketBufferReader,
                // usb_midi_event_packet::UsbMidiEventPacket,
            },
        },
        midi_device::MidiClass,
    };
    use usbd_midi::data::usb::constants::{
        USB_AUDIO_CLASS,
        USB_MIDISTREAMING_SUBCLASS,
    };
    use usb_device::{
        prelude::{UsbDevice},
    };

    static mut EP_MEMORY: [u32; 1024] = [0; 1024];

    // Shared resources go here
    #[shared]
    struct Shared {
        usb: (
            UsbDevice<'static, UsbBus<USB2>>,
            MidiClass<'static, UsbBus<USB2>>,
        ),
    }

    // Local resources go here
    #[local]
    struct Local {
        seed_led: UserLed,
        // timer2: Timer<stm32::TIM2>,
    }

    #[init]
    fn init(cx: init::Context) -> (Shared, Local, init::Monotonics) {
        defmt::info!("init");
        let device = cx.device;

        let board = daisy_bsp::Board::take().unwrap();
        let mut ccdr = board.freeze_clocks(
            device.PWR.constrain(),
            device.RCC.constrain(),
            &device.SYSCFG,
        );
        let _ = ccdr.clocks.hsi48_ck().expect("HSI48 must run");
        ccdr.peripheral.kernel_usb_clk_mux(UsbClkSel::HSI48);

        let pins = board.split_gpios(
            device.GPIOA.split(ccdr.peripheral.GPIOA),
            device.GPIOB.split(ccdr.peripheral.GPIOB),
            device.GPIOC.split(ccdr.peripheral.GPIOC),
            device.GPIOD.split(ccdr.peripheral.GPIOD),
            device.GPIOE.split(ccdr.peripheral.GPIOE),
            device.GPIOF.split(ccdr.peripheral.GPIOF),
            device.GPIOG.split(ccdr.peripheral.GPIOG),
        );

        let mut led_user = UserLed::new(pins.LED_USER);
        defmt::info!("Passed creating led_user");
        led_user.off();
        led_user.on();

        let usb = USB2::new(
            device.OTG2_HS_GLOBAL,
            device.OTG2_HS_DEVICE,
            device.OTG2_HS_PWRCLK,
            pins.USB2.DN.into_alternate(),
            pins.USB2.DP.into_alternate(),
            ccdr.peripheral.USB2OTG,
            &ccdr.clocks,
        );
        defmt::info!("Passed defining USB");

        let usb_bus = cortex_m::singleton!(
            : usb_device::class_prelude::UsbBusAllocator<UsbBus<USB2>> =
                UsbBus::new(usb, unsafe { &mut EP_MEMORY })
        ).unwrap();
        defmt::info!("Passed creating USB bus");

        let midi = MidiClass::new(usb_bus, 1, 1).unwrap();
        defmt::info!("Passed creating MidiClass");

        // TODO: fix here - doesn't go past this build call
        let usb_dev = UsbDeviceBuilder::new(usb_bus, UsbVidPid(0x16c0, 0x5e4))
            .product("daisy midi")
            .device_class(USB_AUDIO_CLASS)
            .device_sub_class(USB_MIDISTREAMING_SUBCLASS)
            // .device_class(USB_CLASS_NONE)
            .build();
        defmt::info!("Passed creating USB device");


        led_user.off();
        // Setup the monotonic timer
        (
            Shared {
                // Initialization of shared resources go here
                usb: (usb_dev, midi),
            },
            Local {
                // Initialization of local resources go here
                seed_led: led_user,
                // timer2,
            },
            init::Monotonics(
                // Initialization of optional monotonic timers go here
            ),
        )
    }

    // Optional idle, can be removed if not needed.
    #[idle]
    fn idle(_: idle::Context) -> ! {
        defmt::info!("idle");

        loop {
            cortex_m::asm::nop();
        }
    }

    #[task(binds = OTG_FS, shared = [usb], local = [seed_led])]
    fn usb_event(mut cx: usb_event::Context) {
        let (local, shared) = (&mut cx.local, &mut cx.shared);
        shared.usb.lock(|(usb_dev, midi)| {
            // let mut led = &local.seed_led;

            if !usb_dev.poll(&mut [midi]) {
                return;
            }

            let mut buffer = [0; 64];
            if let Ok(size) = midi.read(&mut buffer) {
                let buffer_reader = MidiPacketBufferReader::new(&buffer, size);
                for packet in buffer_reader.flatten() {
                    match packet.message {
                        Message::NoteOn(_, _, U7::MIN) | Message::NoteOff(..) => {
                            local.seed_led.off();
                        }
                        Message::NoteOn(..) => {
                            local.seed_led.on();
                        }
                        _ => {}
                    }
                }
            }
        });
    }
}
