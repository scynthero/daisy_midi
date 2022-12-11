#![no_main]
#![no_std]

use daisy_midi as _; // global logger + panicking-behavior + memory layout

#[rtic::app(
device = daisy_bsp::hal::pac, // TODO: Replace `some_hal::pac` with the path to the PAC
dispatchers = [TIM2], // TODO: Replace the `FreeInterrupt1, ...` with free interrupt vectors if software tasks are used
peripherals = true,
)]
mod app {
    // TODO: Add a monotonic if scheduling will be used
    // #[monotonic(binds = SysTick, default = true)]
    // type DwtMono = DwtSystick<80_000_000>;
    use daisy_bsp::hal::{
        // stm32,
        usb_hs::{UsbBus, USB2},
        // timer::{
        //     Event,
        //     Timer
        // },
        device,
        rcc::rec::UsbClkSel};
    use daisy_bsp::led::UserLed;
    use daisy_bsp::hal::prelude::*;
    use daisy_bsp::led::Led;

    // use num_enum::TryFromPrimitive;
    use usb_device::prelude::*;
    use usbd_midi::{
        data::{
            byte::{
                // from_traits::FromClamped,
                u7::U7
            },
            midi::{
                // channel::Channel as MidiChannel,
                message::Message,
                // notes::Note,
            },
            usb::constants::USB_CLASS_NONE,
            usb_midi::{
                midi_packet_reader::MidiPacketBufferReader,
                // usb_midi_event_packet::UsbMidiEventPacket,
            },
        },
        midi_device::MidiClass,
    };

    static mut EP_MEMORY: [u32; 1024] = [0; 1024];

    // Shared resources go here
    #[shared]
    struct Shared {
        // TODO: Add resources
        usb: (
            UsbDevice<'static, UsbBus<USB2>>,
            MidiClass<'static, UsbBus<USB2>>,
        ),
    }

    // Local resources go here
    #[local]
    struct Local {
        // TODO: Add resources
        seed_led: UserLed,
        // timer2: Timer<stm32::TIM2>,
    }

    #[init]
    fn init(cx: init::Context) -> (Shared, Local, init::Monotonics) {
        defmt::info!("init");
        let device = cx.device;

        let board = daisy_bsp::Board::take().unwrap();
        let dp = device::Peripherals::take().unwrap();
        let mut ccdr = board.freeze_clocks(
            dp.PWR.constrain(),
            dp.RCC.constrain(),
            &dp.SYSCFG,
        );
        let _ = ccdr.clocks.hsi48_ck().expect("HSI48 must run");
        ccdr.peripheral.kernel_usb_clk_mux(UsbClkSel::HSI48);

        // let mut timer2 = device
        //     .TIM2
        //     .timer(
        //         fugit::Rate::<u32, 1, 1>::from_raw(20), // 200ms - 1/1*20 Hz
        //         ccdr.peripheral.TIM2, &mut ccdr.clocks,
        //     );
        // timer2.listen(Event::TimeOut);
        let pins = board.split_gpios(
            dp.GPIOA.split(ccdr.peripheral.GPIOA),
            dp.GPIOB.split(ccdr.peripheral.GPIOB),
            dp.GPIOC.split(ccdr.peripheral.GPIOC),
            dp.GPIOD.split(ccdr.peripheral.GPIOD),
            dp.GPIOE.split(ccdr.peripheral.GPIOE),
            dp.GPIOF.split(ccdr.peripheral.GPIOF),
            dp.GPIOG.split(ccdr.peripheral.GPIOG),
        );
        // let gpioa = dp.GPIOA.split(ccdr.peripheral.GPIOA);
        let (pin_dm, pin_dp) = {
            (
                // pins.SEED_PIN_11.into_alternate_af10(),
                // pins.SEED_PIN_12.into_alternate_af10(),
                pins.USB2.DN.into_alternate(),
                pins.USB2.DP.into_alternate(),
            )
        };

        let mut led_user = UserLed::new(pins.LED_USER);
        led_user.off();
        // let mut ccdr = System::init_clocks(device.PWR, device.RCC, &device.SYSCFG);
        // let _ = ccdr.clocks.hsi48_ck().expect("HSI48 must run");
        // ccdr.peripheral.kernel_usb_clk_mux(UsbClkSel::HSI48);

        let usb = USB2::new(
            device.OTG2_HS_GLOBAL,
            device.OTG2_HS_DEVICE,
            device.OTG2_HS_PWRCLK,
            pin_dm,
            pin_dp,
            ccdr.peripheral.USB2OTG,
            &ccdr.clocks,
        );
        let usb_bus = cortex_m::singleton!(
            : usb_device::class_prelude::UsbBusAllocator<UsbBus<USB2>> =
                UsbBus::new(usb, unsafe { &mut EP_MEMORY })
        )
            .unwrap();
        let midi = MidiClass::new(usb_bus, 1, 1).unwrap();

        let usb_dev = UsbDeviceBuilder::new(usb_bus, UsbVidPid(0x16c0, 0x5e4))
            .product("daisy midi")
            .device_class(USB_CLASS_NONE)
            .build();

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
            continue;
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
                                local.seed_led.on();
                            }
                            Message::NoteOn(..) => {
                                local.seed_led.off();
                            }
                            _ => {}
                        }

                }
            }
        });
    }
}
