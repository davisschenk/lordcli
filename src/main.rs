use std::{collections::HashMap, time::Instant};

use clap::{crate_version, App, AppSettings, Arg};
use desert::ToBytes;
use lordserial::{Field, Packet, parser::Lord};
use serialport;

type Error = Box<dyn std::error::Error + Sync + Send>;

fn main() -> Result<(), Error> {
    let matches = App::new("Lord CLI Utility")
        .version(crate_version!())
        .author("Davis Schenkenberger <davis13@colostate.edu>")
        .about("Tools for interacting with Lord Microstrain IMU")
        .setting(AppSettings::ArgRequiredElseHelp)
        .arg(
            Arg::new("PORT")
                .about("The serial port to use")
                .takes_value(true)
                .required(true),
        )
        .subcommand(App::new("test").about("Test the IMU"))
        .subcommand(App::new("configure").about("Configure the IMU"))
        .subcommand(App::new("read").about("Stream data"))
        .subcommand(App::new("list").about("List USB Devices"))
        .subcommand(App::new("rate"))
        .subcommand(App::new("packet"))
        .subcommand(App::new("ekf"))
        .about("Get base rates")
        .get_matches();

    let port_name = matches.value_of("PORT").unwrap();
    let serial = serialport::new(port_name, 115200)
        .open()
        .unwrap_or_else(|e| {
            eprintln!("Failed to open. Error: {}", e);
            ::std::process::exit(0);
        });

    let mut lord = Lord::new(serial);
    lord.start();

    if let Some(_) = matches.subcommand_matches("test") {
        loop {
            if let Some(data) = lord.get_data() {
                println!("{:02X?}", data);
            }
        }
    }

    if let Some(_) = matches.subcommand_matches("rate") {
        println!("IMU Rate: {:#?}", lord.imu_base_rate()?);
        println!("GNSS Rate: {:#?}", lord.gnss_base_rate()?);
    }

    if let Some(_) = matches.subcommand_matches("configure") {
        lord.set_imu_format(
            0x01,
            vec![(0x06, 50), (0x04, 50), (0x05, 50), (0x0A, 50), (0x17, 50)],
        )?;
        println!("IMU Configured");

        lord.set_gnss_format(
            0x01,
            vec![
                (0x09, 5),
                (0x0B, 5),
                (0x03, 5),
                (0x07, 5),
                (0x04, 5)
            ]
        )?;
        println!("GNSS Configured");

    }

    if let Some(_) = matches.subcommand_matches("packet") {
        let packet = Packet::new(
            0x0C,
            vec![
                // Write IMU Format
                Field::new(0x08, vec![
                    0x01, // Function
                    0x05,
                    0x17,
                    0x00, 0x0A,
                    0x06,
                    0x00, 0x0A,
                    0x04,
                    0x00, 0x0A,
                    0x05,
                    0x00, 0x0A,
                    0x0A,
                    0x00, 0x0A,

                ]),
                // Write GNSS Format
                Field::new(0x09, vec![
                    0x01, // Function
                    0x05,
                    0x09,
                    0x00, 0x01,
                    0x0B,
                    0x00, 0x01,
                    0x03,
                    0x00, 0x01,
                    0x07,
                    0x00, 0x01,
                    0x05,
                    0x00, 0x01,
                ]),
                Field::new(0x0A, vec![
                    0x01,
                    0x05,
                    0x11, 
                    0x00, 0x0A,
                    0x01, 
                    0x00, 0x0A,
                    0x02, 
                    0x00, 0x0A, 
                    0x03, 
                    0x00, 0x0A,
                    0x10, 
                    0x00, 0x0A
                ]),

                // Save IMU and GNSS Format
                Field::new(0x08, vec![
                    0x03
                ]),
                Field::new(0x09, vec![
                    0x03
                ]),
                Field::new(0x0A, vec![
                    0x03
                ]),
                // Enable IMU/GNSS Streams/EKF
                Field::new(0x11, vec![
                    0x01,
                    0x01,
                    0x01
                ]),
                Field::new(0x11, vec![
                    0x01,
                    0x02,
                    0x01
                ]),
                Field::new(0x11, vec![
                    0x01,
                    0x03,
                    0x01
                ]),
                // Save stream settings for startup
                Field::new(0x11, vec![0x03, 0x01]),
                Field::new(0x11, vec![0x03, 0x02]),
                Field::new(0x11, vec![0x03, 0x03]),

                Field::new(0x0D, vec![]),
                Field::new(0x19, vec![0x02]),
                Field::new(0x19, vec![0x03, 0x01])


            ]
        );

        println!("{:#02X?}", packet.to_bytes()?);
        match lord.send(packet) {
            Ok(p) => println!("Sent: {:#02X?}", p),
            Err(e) => println!("Error: {:?}", e)
        };        
    }

    if let Some(_) = matches.subcommand_matches("ekf") {
        lord.set_estimation_format(0x01, vec![
            (0x01, 50),
            (0x11, 50)
        ])?;

        lord.set_gnss_format(0x01, vec![
            (0x03, 4),
            (0x09, 4)
        ])?;
        
        lord.send(Packet::new(0x0D, vec![
            Field::new(0x19, vec![0x01, 0x01]),
            Field::new(0x19, vec![0x03, 0x01])
        ]))?;

    }

    if let Some(_) = matches.subcommand_matches("read") {
        let mut seconds_since: HashMap<u8, Instant> = HashMap::new();

        loop {
            if let Some(data) = lord.get_data() {
                let now = Instant::now();
                let ms = match seconds_since.get(&data.header.descriptor) {
                    Some(old) => (now - *old).as_millis(),
                    None => 0,
                };

                seconds_since.insert(data.header.descriptor, now);

                println!("{:02}ms {}", ms, data);

                // if data.header.descriptor == 0x80 {
                //     let field = data.payload.get_field(0x12).unwrap();
                //     println!("GNSS Lat:{:?} Lon:{:?}", field.extract::<f64>(0)?, field.extract::<f64>(0)?);
                //     let field = data.payload.get_field(0x12).unwrap();
                //     println!("TOW: {} WN: {}", field.extract::<f64>(0)?, field.extract::<u16>(8)?);

                // }

                // if data.header.descriptor == 0x82 {
                //     let field = data.payload.get_field(0x01).unwrap();
                //     println!("ESTM Lat:{:?} Lon:{:?}", field.extract::<f64>(0)?, field.extract::<f64>(8)?);
                //     let field = data.payload.get_field(0x11).unwrap();
                //     println!("TOW: {} WN: {}", field.extract::<f64>(0)?, field.extract::<u16>(8)?);

                // }

                // if data.header.descriptor == 0x81 {
                //     let field = data.payload.get_field(0x0B).unwrap();
                //     println!("Fix Type: 0x{:02X?}", field.extract::<u8>(0)?);
                //     println!("SVs: {:?}", field.extract::<u8>(1)?);
                //     println!("Fix Flags: 0x{:04X?}", field.extract::<u16>(2)?);
                //     println!("Valid Flags: 0x{:04X?}", field.extract::<u16>(4)?);
                //     let field = data.payload.get_field(0x09).unwrap();
                //     println!("TOW: {:02X?}", field.extract::<f64>(0)?);
                //     println!("Week Number: {}", field.extract::<u16>(8)?);
                //     println!("Valid Flags: {:016b}", field.extract::<u16>(10)?);
                //     let field = data.payload.get_field(0x03).unwrap();
                //     println!("Lat: {}", field.extract::<f64>(0)?);
                //     println!("Lon: {}", field.extract::<f64>(8)?);

                }
            }
        }
    Ok(())
}
