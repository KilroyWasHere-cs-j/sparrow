pub mod audio;
pub mod rtlsdr;

#[cfg(test)]
mod tests {
    use super::rtlsdr::*;

    // A config that passes validation, used as the starting point for "break one field" tests.
    fn valid() -> RadioConfig {
        RadioConfig::default()
    }

    // ---- Default ----

    #[test]
    fn default_values_are_the_documented_ones() {
        let c = RadioConfig::default();
        assert_eq!(c.device_index, 0);
        assert_eq!(c.frequency_hz, 100_000_000);
        assert_eq!(c.sample_rate, 240_000);
        assert_eq!(c.mode, Mode::Wfm);
        assert_eq!(c.gain, Gain::Auto);
        assert_eq!(c.ppm, 0);
        assert_eq!(c.volume, 0.2);
        assert_eq!(c.squelch, 0.0);
    }

    #[test]
    fn default_config_passes_validation() {
        assert_eq!(RadioConfig::default().validate(), Ok(()));
    }

    // ---- Struct update syntax (the ..Default::default() idiom) ----

    #[test]
    fn struct_update_overrides_only_the_named_fields() {
        let c = RadioConfig {
            frequency_hz: 144_390_000,
            mode: Mode::Nfm,
            gain: Gain::Manual(300),
            ..Default::default()
        };

        assert_eq!(c.frequency_hz, 144_390_000);
        assert_eq!(c.mode, Mode::Nfm);
        assert_eq!(c.gain, Gain::Manual(300));
        // Everything not named keeps its default
        assert_eq!(c.sample_rate, 240_000);
        assert_eq!(c.volume, 0.2);
        assert_eq!(c.validate(), Ok(()));
    }

    // ---- Equality, clone, Copy ----

    #[test]
    fn clone_equals_the_original() {
        let c = RadioConfig {
            mode: Mode::Usb,
            ppm: -3,
            ..Default::default()
        };
        assert_eq!(c.clone(), c);
    }

    #[test]
    fn configs_differing_in_one_field_are_not_equal() {
        let a = valid();
        let variants = [
            RadioConfig {
                device_index: 1,
                ..valid()
            },
            RadioConfig {
                frequency_hz: 101_000_000,
                ..valid()
            },
            RadioConfig {
                sample_rate: 250_000,
                ..valid()
            },
            RadioConfig {
                mode: Mode::Am,
                ..valid()
            },
            RadioConfig {
                gain: Gain::Manual(0),
                ..valid()
            },
            RadioConfig { ppm: 1, ..valid() },
            RadioConfig {
                volume: 0.5,
                ..valid()
            },
            RadioConfig {
                squelch: 0.1,
                ..valid()
            },
        ];
        for v in variants {
            assert_ne!(a, v, "{v:?} should differ from the default");
        }
    }

    #[test]
    fn mode_and_gain_are_copy() {
        // If these stop being Copy, this stops compiling.
        let mode = Mode::Cw;
        let mode_copy = mode;
        assert_eq!(mode, mode_copy);

        let gain = Gain::Manual(100);
        let gain_copy = gain;
        assert_eq!(gain, gain_copy);
    }

    #[test]
    fn all_modes_are_distinct() {
        let modes = [
            Mode::Wfm,
            Mode::Nfm,
            Mode::Am,
            Mode::Usb,
            Mode::Lsb,
            Mode::Cw,
        ];
        for (i, a) in modes.iter().enumerate() {
            for (j, b) in modes.iter().enumerate() {
                assert_eq!(i == j, a == b, "{a:?} vs {b:?}");
            }
        }
    }

    #[test]
    fn gain_manual_compares_by_value() {
        assert_eq!(Gain::Manual(300), Gain::Manual(300));
        assert_ne!(Gain::Manual(300), Gain::Manual(301));
        assert_ne!(Gain::Auto, Gain::Manual(0));
    }

    #[test]
    fn debug_output_names_the_fields() {
        let text = format!("{:?}", valid());
        assert!(text.contains("frequency_hz"), "{text}");
        assert!(text.contains("Wfm"), "{text}");
        assert!(text.contains("Auto"), "{text}");
    }

    // ---- validate: frequency ----

    #[test]
    fn frequency_lower_edge_is_inclusive() {
        let c = RadioConfig {
            frequency_hz: 24_000_000,
            ..valid()
        };
        assert_eq!(c.validate(), Ok(()));
    }

    #[test]
    fn frequency_just_below_range_is_rejected() {
        let c = RadioConfig {
            frequency_hz: 23_999_999,
            ..valid()
        };
        let err = c.validate().unwrap_err();
        assert!(err.contains("23999999"), "{err}");
        assert!(err.contains("24-1700 MHz"), "{err}");
    }

    #[test]
    fn frequency_upper_edge_is_inclusive() {
        let c = RadioConfig {
            frequency_hz: 1_700_000_000,
            ..valid()
        };
        assert_eq!(c.validate(), Ok(()));
    }

    #[test]
    fn frequency_just_above_range_is_rejected() {
        let c = RadioConfig {
            frequency_hz: 1_700_000_001,
            ..valid()
        };
        assert!(c.validate().unwrap_err().contains("tuning range"));
    }

    #[test]
    fn frequency_zero_and_max_are_rejected() {
        for hz in [0, u32::MAX] {
            let c = RadioConfig {
                frequency_hz: hz,
                ..valid()
            };
            assert!(c.validate().is_err(), "{hz} Hz should be rejected");
        }
    }

    #[test]
    fn common_ham_and_broadcast_frequencies_are_accepted() {
        // FM broadcast, APRS (US and Europe), 2 m calling frequency, airband
        for hz in [
            98_100_000,
            144_390_000,
            144_800_000,
            146_520_000,
            121_500_000,
        ] {
            let c = RadioConfig {
                frequency_hz: hz,
                ..valid()
            };
            assert_eq!(c.validate(), Ok(()), "{hz} Hz");
        }
    }

    // ---- validate: sample rate ----

    #[test]
    fn sample_rate_lower_edge_is_225001() {
        let ok = RadioConfig {
            sample_rate: 225_001,
            ..valid()
        };
        assert_eq!(ok.validate(), Ok(()));

        let bad = RadioConfig {
            sample_rate: 225_000,
            ..valid()
        };
        assert!(bad.validate().unwrap_err().contains("sample rate"));
    }

    #[test]
    fn sample_rate_upper_edge_is_3_2_million() {
        let ok = RadioConfig {
            sample_rate: 3_200_000,
            ..valid()
        };
        assert_eq!(ok.validate(), Ok(()));

        let bad = RadioConfig {
            sample_rate: 3_200_001,
            ..valid()
        };
        assert!(bad.validate().unwrap_err().contains("3200001"));
    }

    #[test]
    fn sample_rate_zero_is_rejected() {
        let c = RadioConfig {
            sample_rate: 0,
            ..valid()
        };
        assert!(c.validate().is_err());
    }

    // ---- validate: volume ----

    #[test]
    fn volume_edges_are_inclusive() {
        for v in [0.0, 1.0] {
            let c = RadioConfig {
                volume: v,
                ..valid()
            };
            assert_eq!(c.validate(), Ok(()), "volume {v}");
        }
    }

    #[test]
    fn volume_outside_zero_to_one_is_rejected() {
        for v in [-0.01, 1.01, -5.0, 100.0] {
            let c = RadioConfig {
                volume: v,
                ..valid()
            };
            let err = c.validate().unwrap_err();
            assert!(err.contains("volume"), "volume {v}: {err}");
        }
    }

    #[test]
    fn volume_nan_and_infinity_are_rejected() {
        for v in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let c = RadioConfig {
                volume: v,
                ..valid()
            };
            assert!(c.validate().is_err(), "volume {v} should be rejected");
        }
    }

    // ---- validate: squelch ----

    #[test]
    fn squelch_edges_are_inclusive() {
        for s in [0.0, 1.0] {
            let c = RadioConfig {
                squelch: s,
                ..valid()
            };
            assert_eq!(c.validate(), Ok(()), "squelch {s}");
        }
    }

    #[test]
    fn squelch_outside_zero_to_one_is_rejected() {
        for s in [-0.01, 1.01] {
            let c = RadioConfig {
                squelch: s,
                ..valid()
            };
            let err = c.validate().unwrap_err();
            assert!(err.contains("squelch"), "squelch {s}: {err}");
        }
    }

    #[test]
    fn squelch_nan_is_rejected() {
        let c = RadioConfig {
            squelch: f32::NAN,
            ..valid()
        };
        assert!(c.validate().is_err());
    }

    // ---- validate: fields that are deliberately not checked ----

    #[test]
    fn ppm_is_not_range_checked() {
        for ppm in [i32::MIN, -50, 0, 50, i32::MAX] {
            let c = RadioConfig { ppm, ..valid() };
            assert_eq!(c.validate(), Ok(()), "ppm {ppm}");
        }
    }

    #[test]
    fn device_index_is_not_checked() {
        let c = RadioConfig {
            device_index: u32::MAX,
            ..valid()
        };
        assert_eq!(c.validate(), Ok(()));
    }

    #[test]
    fn every_mode_validates() {
        for mode in [
            Mode::Wfm,
            Mode::Nfm,
            Mode::Am,
            Mode::Usb,
            Mode::Lsb,
            Mode::Cw,
        ] {
            let c = RadioConfig { mode, ..valid() };
            assert_eq!(c.validate(), Ok(()), "{mode:?}");
        }
    }

    #[test]
    fn any_gain_validates() {
        // Gain is not range-checked here: the tuner snaps to the nearest step it supports.
        for gain in [
            Gain::Auto,
            Gain::Manual(0),
            Gain::Manual(-100),
            Gain::Manual(496),
            Gain::Manual(i32::MAX),
        ] {
            let c = RadioConfig { gain, ..valid() };
            assert_eq!(c.validate(), Ok(()), "{gain:?}");
        }
    }

    // ---- validate: which error wins when several things are wrong ----

    #[test]
    fn first_bad_field_is_reported_in_check_order() {
        // Order in validate(): frequency, sample rate, volume, squelch.
        let all_bad = RadioConfig {
            frequency_hz: 0,
            sample_rate: 0,
            volume: 2.0,
            squelch: 2.0,
            ..valid()
        };
        assert!(all_bad.validate().unwrap_err().contains("tuning range"));

        let no_freq = RadioConfig {
            frequency_hz: 100_000_000,
            ..all_bad.clone()
        };
        assert!(no_freq.validate().unwrap_err().contains("sample rate"));

        let no_rate = RadioConfig {
            sample_rate: 240_000,
            ..no_freq.clone()
        };
        assert!(no_rate.validate().unwrap_err().contains("volume"));

        let no_volume = RadioConfig {
            volume: 0.5,
            ..no_rate.clone()
        };
        assert!(no_volume.validate().unwrap_err().contains("squelch"));

        let all_good = RadioConfig {
            squelch: 0.5,
            ..no_volume
        };
        assert_eq!(all_good.validate(), Ok(()));
    }

    #[test]
    fn validate_does_not_change_the_config() {
        let c = RadioConfig {
            volume: 5.0,
            ..valid()
        };
        let before = c.clone();
        let _ = c.validate();
        assert_eq!(c, before);
    }
}
