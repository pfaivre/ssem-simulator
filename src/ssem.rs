//! A simulator for the Small-Scale Experimental Machine

pub(self) mod opcode;
pub mod simulator;
pub(self) mod store;

mod tests {
    // TODO: move these tests into tests/ folder
    #[test]
    fn integration_test_all() {
        let initial_store_start: Vec<String> = vec![
            "10000000000000000000000000000000".into(), // 00
            "01010000000000100000000000000000".into(), // 01
            "00000000000000010000000000000000".into(), // 02
            "00001000000001100000000000000000".into(), // 03
            "00000000000000110000000000000000".into(), // 04
            "00000000000000000000000000000000".into(), // 05
            "00000000000001000000000000000000".into(), // 06
            "00000000000000000000000000000000".into(), // 07
            "00000000000001110000000000000000".into(), // 08
            "00000000000000000000000000000000".into(), // 09
            "00000111111111111111111111111111".into(), // 10
            "00000000000000000000000000000000".into(), // 11
            "00000000000000000000000000000000".into(), // 12
            "00000000000000000000000000000000".into(), // 13
            "00000000000000000000000000000000".into(), // 14
            "00000000000000000000000000000000".into(), // 15
            "00000000000000000000000000000000".into(), // 16
            "00000000000000000000000000000000".into(), // 17
            "00000000000000000000000000000000".into(), // 18
            "00000000000000000000000000000000".into(), // 19
            "00000000000000000000000000000000".into(), // 20
            "00000000000000000000000000000000".into(), // 21
            "00000000000000000000000000000000".into(), // 22
            "00000000000000000000000000000000".into(), // 23
            "00000000000000000000000000000000".into(), // 24
            "00000000000000000000000000000000".into(), // 25
            "00000000000000000000000000000000".into(), // 26
            "00000000000000000000000000000000".into(), // 27
            "00000000000000000000000000000000".into(), // 28
            "00000000000000000000000000000000".into(), // 29
            "00000000000000000000000000000000".into(), // 30
            "00000000000000000000000000000000".into(), // 31
        ];
        let expected_store_end: Vec<String> = vec![
            "10000000000000000000000000000000".into(), // 00
            "01010000000000100000000000000000".into(), // 01
            "00000000000000010000000000000000".into(), // 02
            "00001000000001100000000000000000".into(), // 03
            "00000000000000110000000000000000".into(), // 04
            "00000000000000000000000000000000".into(), // 05
            "00000000000001000000000000000000".into(), // 06
            "00000000000000000000000000000000".into(), // 07
            "00000000000001110000000000000000".into(), // 08
            "00000000000000000000000000000000".into(), // 09
            "00000111111111111111111111111111".into(), // 10
            "00000000000000000000000000000000".into(), // 11
            "00000000000000000000000000000000".into(), // 12
            "00000000000000000000000000000000".into(), // 13
            "00000000000000000000000000000000".into(), // 14
            "00000000000000000000000000000000".into(), // 15
            "11111111111111111111111111111111".into(), // 16 < changes here
            "00000000000000000000000000000000".into(), // 17
            "00000000000000000000000000000000".into(), // 18
            "00000000000000000000000000000000".into(), // 19
            "00000000000000000000000000000000".into(), // 20
            "00000000000000000000000000000000".into(), // 21
            "00000000000000000000000000000000".into(), // 22
            "00000000000000000000000000000000".into(), // 23
            "00000000000000000000000000000000".into(), // 24
            "00000000000000000000000000000000".into(), // 25
            "00000000000000000000000000000000".into(), // 26
            "00000000000000000000000000000000".into(), // 27
            "00000000000000000000000000000000".into(), // 28
            "00000000000000000000000000000000".into(), // 29
            "00000000000000000000000000000000".into(), // 30
            "00000000000000000000000000000000".into(), // 31
        ];

        let mut simulator = crate::ssem::simulator::Simulator::from(initial_store_start);
        simulator.run(300);
        let expected_end_state = crate::ssem::simulator::Simulator::from(expected_store_end);
        assert_eq!(simulator.store, expected_end_state.store);
    }

    #[test]
    fn integration_test_cmp1() {
        let initial_store_start: Vec<String> = vec![
            "00000000000000000000000000000000".into(), // 00
            "11111000000000100000000000000000".into(), // 01
            "01111000000001100000000000000000".into(), // 02
            "01111000000000100000000000000000".into(), // 03
            "11001000000000010000000000000000".into(), // 04
            "11111000000001100000000000000000".into(), // 05
            "00000000000000110000000000000000".into(), // 06
            "11001000000001000000000000000000".into(), // 07
            "00000000000001110000000000000000".into(), // 08
            "00000000000000000000000000000000".into(), // 09
            "00000000000000000000000000000000".into(), // 10
            "00000000000000000000000000000000".into(), // 11
            "00000000000000000000000000000000".into(), // 12
            "00000000000000000000000000000000".into(), // 13
            "00000000000000000000000000000000".into(), // 14
            "00000000000000000000000000000000".into(), // 15
            "00000000000000000000000000000000".into(), // 16
            "00000000000000000000000000000000".into(), // 17
            "00000000000000000000000000000000".into(), // 18
            "10000000000000000000000000000000".into(), // 19
            "00000000000000000000000000000000".into(), // 20
            "00000000000000000000000000000000".into(), // 21
            "00000000000000000000000000000000".into(), // 22
            "00000000000000000000000000000000".into(), // 23
            "00000000000000000000000000000000".into(), // 24
            "00000000000000000000000000000000".into(), // 25
            "00000000000000000000000000000000".into(), // 26
            "00000000000000000000000000000000".into(), // 27
            "00000000000000000000000000000000".into(), // 28
            "00000000000000000000000000000000".into(), // 29
            "00000000000000000000000000000000".into(), // 30
            "00000100000000000000000000000000".into(), // 31
        ];
        let expected_store_end: Vec<String> = vec![
            "00000000000000000000000000000000".into(), // 00
            "11111000000000100000000000000000".into(), // 01
            "01111000000001100000000000000000".into(), // 02
            "01111000000000100000000000000000".into(), // 03
            "11001000000000010000000000000000".into(), // 04
            "11111000000001100000000000000000".into(), // 05
            "00000000000000110000000000000000".into(), // 06
            "11001000000001000000000000000000".into(), // 07
            "00000000000001110000000000000000".into(), // 08
            "00000000000000000000000000000000".into(), // 09
            "00000000000000000000000000000000".into(), // 10
            "00000000000000000000000000000000".into(), // 11
            "00000000000000000000000000000000".into(), // 12
            "00000000000000000000000000000000".into(), // 13
            "00000000000000000000000000000000".into(), // 14
            "00000000000000000000000000000000".into(), // 15
            "00000000000000000000000000000000".into(), // 16
            "00000000000000000000000000000000".into(), // 17
            "00000000000000000000000000000000".into(), // 18
            "10000000000000000000000000000000".into(), // 19
            "00000000000000000000000000000000".into(), // 20
            "00000000000000000000000000000000".into(), // 21
            "00000000000000000000000000000000".into(), // 22
            "00000000000000000000000000000000".into(), // 23
            "00000000000000000000000000000000".into(), // 24
            "00000000000000000000000000000000".into(), // 25
            "00000000000000000000000000000000".into(), // 26
            "00000000000000000000000000000000".into(), // 27
            "00000000000000000000000000000000".into(), // 28
            "00000000000000000000000000000000".into(), // 29
            "00000000000000000000000000000000".into(), // 30
            "11111111111111111111111111111111".into(), // 31 < changes here
        ];

        let mut simulator = crate::ssem::simulator::Simulator::from(initial_store_start);
        simulator.run(300);
        let expected_end_state = crate::ssem::simulator::Simulator::from(expected_store_end);
        assert_eq!(simulator.store, expected_end_state.store);
    }

    #[test]
    fn integration_test_cmp2() {
        let initial_store_start: Vec<String> = vec![
            "00000000000000000000000000000000".into(), // 00
            "11111000000000100000000000000000".into(), // 01
            "11001000000000010000000000000000".into(), // 02
            "01111000000001100000000000000000".into(), // 03
            "01111000000000100000000000000000".into(), // 04
            "11111000000001100000000000000000".into(), // 05
            "00000000000000110000000000000000".into(), // 06
            "00000000000001110000000000000000".into(), // 07
            "00000000000000000000000000000000".into(), // 08
            "00000000000000000000000000000000".into(), // 09
            "00000000000000000000000000000000".into(), // 10
            "00000000000000000000000000000000".into(), // 11
            "00000000000000000000000000000000".into(), // 12
            "00000000000000000000000000000000".into(), // 13
            "00000000000000000000000000000000".into(), // 14
            "00000000000000000000000000000000".into(), // 15
            "00000000000000000000000000000000".into(), // 16
            "00000000000000000000000000000000".into(), // 17
            "00000000000000000000000000000000".into(), // 18
            "10000000000000000000000000000000".into(), // 19
            "00000000000000000000000000000000".into(), // 20
            "00000000000000000000000000000000".into(), // 21
            "00000000000000000000000000000000".into(), // 22
            "00000000000000000000000000000000".into(), // 23
            "00000000000000000000000000000000".into(), // 24
            "00000000000000000000000000000000".into(), // 25
            "00000000000000000000000000000000".into(), // 26
            "00000000000000000000000000000000".into(), // 27
            "00000000000000000000000000000000".into(), // 28
            "00000000000000000000000000000000".into(), // 29
            "00000000000000000000000000000000".into(), // 30
            "00000111111111111111111111111111".into(), // 31
        ];
        let expected_store_end: Vec<String> = vec![
            "00000000000000000000000000000000".into(), // 00
            "11111000000000100000000000000000".into(), // 01
            "11001000000000010000000000000000".into(), // 02
            "01111000000001100000000000000000".into(), // 03
            "01111000000000100000000000000000".into(), // 04
            "11111000000001100000000000000000".into(), // 05
            "00000000000000110000000000000000".into(), // 06
            "00000000000001110000000000000000".into(), // 07
            "00000000000000000000000000000000".into(), // 08
            "00000000000000000000000000000000".into(), // 09
            "00000000000000000000000000000000".into(), // 10
            "00000000000000000000000000000000".into(), // 11
            "00000000000000000000000000000000".into(), // 12
            "00000000000000000000000000000000".into(), // 13
            "00000000000000000000000000000000".into(), // 14
            "00000000000000000000000000000000".into(), // 15
            "00000000000000000000000000000000".into(), // 16
            "00000000000000000000000000000000".into(), // 17
            "00000000000000000000000000000000".into(), // 18
            "10000000000000000000000000000000".into(), // 19
            "00000000000000000000000000000000".into(), // 20
            "00000000000000000000000000000000".into(), // 21
            "00000000000000000000000000000000".into(), // 22
            "00000000000000000000000000000000".into(), // 23
            "00000000000000000000000000000000".into(), // 24
            "00000000000000000000000000000000".into(), // 25
            "00000000000000000000000000000000".into(), // 26
            "00000000000000000000000000000000".into(), // 27
            "00000000000000000000000000000000".into(), // 28
            "00000000000000000000000000000000".into(), // 29
            "00000000000000000000000000000000".into(), // 30
            "00000000000000000000000000000000".into(), // 31 < changes here
        ];

        let mut simulator = crate::ssem::simulator::Simulator::from(initial_store_start);
        simulator.run(300);
        let expected_end_state = crate::ssem::simulator::Simulator::from(expected_store_end);
        assert_eq!(simulator.store, expected_end_state.store);
    }

    #[test]
    fn integration_test_jmp1() {
        let initial_store_start: Vec<String> = vec![
            "00000000000000000000000000000000".into(), // 00
            "10000000000000000000000000000000".into(), // 01
            "01000000000000000000000000000000".into(), // 02
            "11000000000000000000000000000000".into(), // 03
            "00100000000000000000000000000000".into(), // 04
            "10100000000000000000000000000000".into(), // 05
            "01100000000000000000000000000000".into(), // 06
            "11100000000000000000000000000000".into(), // 07
            "00010000000000000000000000000000".into(), // 08
            "10010000000000000000000000000000".into(), // 09
            "01010000000000000000000000000000".into(), // 10
            "11010000000000000000000000000000".into(), // 11
            "00110000000000000000000000000000".into(), // 12
            "10110000000000000000000000000000".into(), // 13
            "01110000000000000000000000000000".into(), // 14
            "11110000000000000000000000000000".into(), // 15
            "00001000000000000000000000000000".into(), // 16
            "10001000000000000000000000000000".into(), // 17
            "01001000000000000000000000000000".into(), // 18
            "11001000000000000000000000000000".into(), // 19
            "00101000000000000000000000000000".into(), // 20
            "10101000000000000000000000000000".into(), // 21
            "01101000000000000000000000000000".into(), // 22
            "11101000000000000000000000000000".into(), // 23
            "00011000000000000000000000000000".into(), // 24
            "10011000000000000000000000000000".into(), // 25
            "01011000000000000000000000000000".into(), // 26
            "11011000000000000000000000000000".into(), // 27
            "00111000000000000000000000000000".into(), // 28
            "10111000000000000000000000000000".into(), // 29
            "01111000000000000000000000000000".into(), // 30
            "11111000000000000000000000000000".into(), // 31
        ];
        let expected_store_end: Vec<String> = vec![
            "00000000000000000000000000000000".into(), // 00
            "10000000000000000000000000000000".into(), // 01
            "01000000000000000000000000000000".into(), // 02
            "11000000000000000000000000000000".into(), // 03
            "00100000000000000000000000000000".into(), // 04
            "10100000000000000000000000000000".into(), // 05
            "01100000000000000000000000000000".into(), // 06
            "11100000000000000000000000000000".into(), // 07
            "00010000000000000000000000000000".into(), // 08
            "10010000000000000000000000000000".into(), // 09
            "01010000000000000000000000000000".into(), // 10
            "11010000000000000000000000000000".into(), // 11
            "00110000000000000000000000000000".into(), // 12
            "10110000000000000000000000000000".into(), // 13
            "01110000000000000000000000000000".into(), // 14
            "11110000000000000000000000000000".into(), // 15
            "00001000000000000000000000000000".into(), // 16
            "10001000000000000000000000000000".into(), // 17
            "01001000000000000000000000000000".into(), // 18
            "11001000000000000000000000000000".into(), // 19
            "00101000000000000000000000000000".into(), // 20
            "10101000000000000000000000000000".into(), // 21
            "01101000000000000000000000000000".into(), // 22
            "11101000000000000000000000000000".into(), // 23
            "00011000000000000000000000000000".into(), // 24
            "10011000000000000000000000000000".into(), // 25
            "01011000000000000000000000000000".into(), // 26
            "11011000000000000000000000000000".into(), // 27
            "00111000000000000000000000000000".into(), // 28
            "10111000000000000000000000000000".into(), // 29
            "01111000000000000000000000000000".into(), // 30
            "11111000000000000000000000000000".into(), // 31
        ];

        let mut simulator = crate::ssem::simulator::Simulator::from(initial_store_start);
        simulator.run(31);
        let expected_end_state = crate::ssem::simulator::Simulator::from(expected_store_end);
        assert_eq!(simulator.store, expected_end_state.store);
        assert_eq!(
            "11111000000000000000000000000000",
            format!("{:032b}", simulator.ci.reverse_bits())
        )
    }

    #[test]
    fn integration_test_jrp1() {
        let initial_store_start: Vec<String> = vec![
            "00000000000000000000000000000000".into(), // 00
            "01111000000001000000000000000000".into(), // 01
            "11111000000001000000000000000000".into(), // 02
            "11111000000001000000000000000000".into(), // 03
            "11111000000001000000000000000000".into(), // 04
            "11111000000001000000000000000000".into(), // 05
            "11111000000001000000000000000000".into(), // 06
            "11111000000001000000000000000000".into(), // 07
            "11111000000001000000000000000000".into(), // 08
            "11111000000001000000000000000000".into(), // 09
            "11111000000001000000000000000000".into(), // 10
            "11111000000001000000000000000000".into(), // 11
            "11111000000001000000000000000000".into(), // 12
            "11111000000001000000000000000000".into(), // 13
            "11111000000001000000000000000000".into(), // 14
            "11111000000001000000000000000000".into(), // 15
            "11111000000001000000000000000000".into(), // 16
            "11111000000001000000000000000000".into(), // 17
            "11111000000001000000000000000000".into(), // 18
            "11111000000001000000000000000000".into(), // 19
            "11111000000001000000000000000000".into(), // 20
            "11111000000001000000000000000000".into(), // 21
            "11111000000001000000000000000000".into(), // 22
            "11111000000001000000000000000000".into(), // 23
            "11111000000001000000000000000000".into(), // 24
            "11111000000001000000000000000000".into(), // 25
            "11111000000001000000000000000000".into(), // 26
            "11111000000001000000000000000000".into(), // 27
            "11111000000001000000000000000000".into(), // 28
            "11111000000001000000000000000000".into(), // 29
            "11011000000000000000000000000000".into(), // 30
            "01111111111111111111111111111111".into(), // 31
        ];
        let expected_store_end: Vec<String> = vec![
            "00000000000000000000000000000000".into(), // 00
            "01111000000001000000000000000000".into(), // 01
            "11111000000001000000000000000000".into(), // 02
            "11111000000001000000000000000000".into(), // 03
            "11111000000001000000000000000000".into(), // 04
            "11111000000001000000000000000000".into(), // 05
            "11111000000001000000000000000000".into(), // 06
            "11111000000001000000000000000000".into(), // 07
            "11111000000001000000000000000000".into(), // 08
            "11111000000001000000000000000000".into(), // 09
            "11111000000001000000000000000000".into(), // 10
            "11111000000001000000000000000000".into(), // 11
            "11111000000001000000000000000000".into(), // 12
            "11111000000001000000000000000000".into(), // 13
            "11111000000001000000000000000000".into(), // 14
            "11111000000001000000000000000000".into(), // 15
            "11111000000001000000000000000000".into(), // 16
            "11111000000001000000000000000000".into(), // 17
            "11111000000001000000000000000000".into(), // 18
            "11111000000001000000000000000000".into(), // 19
            "11111000000001000000000000000000".into(), // 20
            "11111000000001000000000000000000".into(), // 21
            "11111000000001000000000000000000".into(), // 22
            "11111000000001000000000000000000".into(), // 23
            "11111000000001000000000000000000".into(), // 24
            "11111000000001000000000000000000".into(), // 25
            "11111000000001000000000000000000".into(), // 26
            "11111000000001000000000000000000".into(), // 27
            "11111000000001000000000000000000".into(), // 28
            "11111000000001000000000000000000".into(), // 29
            "11011000000000000000000000000000".into(), // 30
            "01111111111111111111111111111111".into(), // 31
        ];

        let mut simulator = crate::ssem::simulator::Simulator::from(initial_store_start);
        simulator.run(30);
        let expected_end_state = crate::ssem::simulator::Simulator::from(expected_store_end);
        assert_eq!(simulator.store, expected_end_state.store);
        assert_eq!(
            "00111000000000000000000000000000",
            format!("{:032b}", simulator.ci.reverse_bits())
        )
    }

    #[test]
    fn integration_test_ldn1() {
        let initial_store_start: Vec<String> = vec![
            "00000000000000000000000000000000".into(), // 00
            "10000000000000100000000000000000".into(), // 01
            "01000000000000100000000000000000".into(), // 02
            "11000000000000100000000000000000".into(), // 03
            "00100000000000100000000000000000".into(), // 04
            "10100000000000100000000000000000".into(), // 05
            "01100000000000100000000000000000".into(), // 06
            "11100000000000100000000000000000".into(), // 07
            "00010000000000100000000000000000".into(), // 08
            "10010000000000100000000000000000".into(), // 09
            "01010000000000100000000000000000".into(), // 10
            "11010000000000100000000000000000".into(), // 11
            "00110000000000100000000000000000".into(), // 12
            "10110000000000100000000000000000".into(), // 13
            "01110000000000100000000000000000".into(), // 14
            "11110000000000100000000000000000".into(), // 15
            "00001000000000100000000000000000".into(), // 16
            "10001000000000100000000000000000".into(), // 17
            "01001000000000100000000000000000".into(), // 18
            "11001000000000100000000000000000".into(), // 19
            "00101000000000100000000000000000".into(), // 20
            "10101000000000100000000000000000".into(), // 21
            "01101000000000100000000000000000".into(), // 22
            "11101000000000100000000000000000".into(), // 23
            "00011000000000100000000000000000".into(), // 24
            "10011000000000100000000000000000".into(), // 25
            "01011000000000100000000000000000".into(), // 26
            "11011000000000100000000000000000".into(), // 27
            "00111000000000100000000000000000".into(), // 28
            "10111000000000100000000000000000".into(), // 29
            "01111000000000100000000000000000".into(), // 30
            "11111000000000100000000000000000".into(), // 31
        ];
        let expected_store_end: Vec<String> = vec![
            "00000000000000000000000000000000".into(), // 00
            "10000000000000100000000000000000".into(), // 01
            "01000000000000100000000000000000".into(), // 02
            "11000000000000100000000000000000".into(), // 03
            "00100000000000100000000000000000".into(), // 04
            "10100000000000100000000000000000".into(), // 05
            "01100000000000100000000000000000".into(), // 06
            "11100000000000100000000000000000".into(), // 07
            "00010000000000100000000000000000".into(), // 08
            "10010000000000100000000000000000".into(), // 09
            "01010000000000100000000000000000".into(), // 10
            "11010000000000100000000000000000".into(), // 11
            "00110000000000100000000000000000".into(), // 12
            "10110000000000100000000000000000".into(), // 13
            "01110000000000100000000000000000".into(), // 14
            "11110000000000100000000000000000".into(), // 15
            "00001000000000100000000000000000".into(), // 16
            "10001000000000100000000000000000".into(), // 17
            "01001000000000100000000000000000".into(), // 18
            "11001000000000100000000000000000".into(), // 19
            "00101000000000100000000000000000".into(), // 20
            "10101000000000100000000000000000".into(), // 21
            "01101000000000100000000000000000".into(), // 22
            "11101000000000100000000000000000".into(), // 23
            "00011000000000100000000000000000".into(), // 24
            "10011000000000100000000000000000".into(), // 25
            "01011000000000100000000000000000".into(), // 26
            "11011000000000100000000000000000".into(), // 27
            "00111000000000100000000000000000".into(), // 28
            "10111000000000100000000000000000".into(), // 29
            "01111000000000100000000000000000".into(), // 30
            "11111000000000100000000000000000".into(), // 31
        ];

        let mut simulator = crate::ssem::simulator::Simulator::from(initial_store_start);
        simulator.run(35);
        let expected_end_state = crate::ssem::simulator::Simulator::from(expected_store_end);
        assert_eq!(simulator.store, expected_end_state.store);
        assert_eq!(
            "10111111111111011111111111111111",
            format!("{:032b}", simulator.a.reverse_bits())
        )
    }

    #[test]
    fn integration_test_sto1() {
        let initial_store_start: Vec<String> = vec![
            "00000000000000000000000000000000".into(), // 00
            "11111000000000100000000000000000".into(), // 01
            "01000000000001100000000000000000".into(), // 02
            "11000000000001100000000000000000".into(), // 03
            "00100000000001100000000000000000".into(), // 04
            "10100000000001100000000000000000".into(), // 05
            "01100000000001100000000000000000".into(), // 06
            "11100000000001100000000000000000".into(), // 07
            "00010000000001100000000000000000".into(), // 08
            "10010000000001100000000000000000".into(), // 09
            "01010000000001100000000000000000".into(), // 10
            "11010000000001100000000000000000".into(), // 11
            "00110000000001100000000000000000".into(), // 12
            "10110000000001100000000000000000".into(), // 13
            "01110000000001100000000000000000".into(), // 14
            "11110000000001100000000000000000".into(), // 15
            "00001000000001100000000000000000".into(), // 16
            "10001000000001100000000000000000".into(), // 17
            "01001000000001100000000000000000".into(), // 18
            "11001000000001100000000000000000".into(), // 19
            "00101000000001100000000000000000".into(), // 20
            "10101000000001100000000000000000".into(), // 21
            "01101000000001100000000000000000".into(), // 22
            "11101000000001100000000000000000".into(), // 23
            "00011000000001100000000000000000".into(), // 24
            "10011000000001100000000000000000".into(), // 25
            "01011000000001100000000000000000".into(), // 26
            "11011000000001100000000000000000".into(), // 27
            "00111000000001100000000000000000".into(), // 28
            "10111000000001100000000000000000".into(), // 29
            "00000000000001110000000000000000".into(), // 30
            "11010101010101010101010101010101".into(), // 31
        ];
        let expected_store_end: Vec<String> = vec![
            "00000000000000000000000000000000".into(), // 00
            "11111000000000100000000000000000".into(), // 01
            "10101010101010101010101010101010".into(), // 02 < changes here
            "10101010101010101010101010101010".into(), // 03 < changes here
            "10101010101010101010101010101010".into(), // 04 < changes here
            "10101010101010101010101010101010".into(), // 05 < changes here
            "10101010101010101010101010101010".into(), // 06 < changes here
            "10101010101010101010101010101010".into(), // 07 < changes here
            "10101010101010101010101010101010".into(), // 08 < changes here
            "10101010101010101010101010101010".into(), // 09 < changes here
            "10101010101010101010101010101010".into(), // 10 < changes here
            "10101010101010101010101010101010".into(), // 11 < changes here
            "10101010101010101010101010101010".into(), // 12 < changes here
            "10101010101010101010101010101010".into(), // 13 < changes here
            "10101010101010101010101010101010".into(), // 14 < changes here
            "10101010101010101010101010101010".into(), // 15 < changes here
            "10101010101010101010101010101010".into(), // 16 < changes here
            "10101010101010101010101010101010".into(), // 17 < changes here
            "10101010101010101010101010101010".into(), // 18 < changes here
            "10101010101010101010101010101010".into(), // 19 < changes here
            "10101010101010101010101010101010".into(), // 20 < changes here
            "10101010101010101010101010101010".into(), // 21 < changes here
            "10101010101010101010101010101010".into(), // 22 < changes here
            "10101010101010101010101010101010".into(), // 23 < changes here
            "10101010101010101010101010101010".into(), // 24 < changes here
            "10101010101010101010101010101010".into(), // 25 < changes here
            "10101010101010101010101010101010".into(), // 26 < changes here
            "10101010101010101010101010101010".into(), // 27 < changes here
            "10101010101010101010101010101010".into(), // 28 < changes here
            "10101010101010101010101010101010".into(), // 29 < changes here
            "00000000000001110000000000000000".into(), // 30
            "11010101010101010101010101010101".into(), // 31
        ];

        let mut simulator = crate::ssem::simulator::Simulator::from(initial_store_start);
        simulator.run(29);
        let expected_end_state = crate::ssem::simulator::Simulator::from(expected_store_end);
        assert_eq!(simulator.store, expected_end_state.store);
    }

    #[test]
    fn integration_test_sto2() {
        let initial_store_start: Vec<String> = vec![
            "00000000000000000000000000000000".into(), // 00
            "11111000000000100000000000000000".into(), // 01
            "01000000000001100000000000000000".into(), // 02
            "11000000000001100000000000000000".into(), // 03
            "00100000000001100000000000000000".into(), // 04
            "10100000000001100000000000000000".into(), // 05
            "01100000000001100000000000000000".into(), // 06
            "11100000000001100000000000000000".into(), // 07
            "00010000000001100000000000000000".into(), // 08
            "10010000000001100000000000000000".into(), // 09
            "01010000000001100000000000000000".into(), // 10
            "11010000000001100000000000000000".into(), // 11
            "00110000000001100000000000000000".into(), // 12
            "10110000000001100000000000000000".into(), // 13
            "01110000000001100000000000000000".into(), // 14
            "11110000000001100000000000000000".into(), // 15
            "00001000000001100000000000000000".into(), // 16
            "10001000000001100000000000000000".into(), // 17
            "01001000000001100000000000000000".into(), // 18
            "11001000000001100000000000000000".into(), // 19
            "00101000000001100000000000000000".into(), // 20
            "10101000000001100000000000000000".into(), // 21
            "01101000000001100000000000000000".into(), // 22
            "11101000000001100000000000000000".into(), // 23
            "00011000000001100000000000000000".into(), // 24
            "10011000000001100000000000000000".into(), // 25
            "01011000000001100000000000000000".into(), // 26
            "11011000000001100000000000000000".into(), // 27
            "00111000000001100000000000000000".into(), // 28
            "10111000000001100000000000000000".into(), // 29
            "00000000000001110000000000000000".into(), // 30
            "01101010101010101010101010101010".into(), // 31
        ];
        let expected_store_end: Vec<String> = vec![
            "00000000000000000000000000000000".into(), // 00
            "11111000000000100000000000000000".into(), // 01
            "01010101010101010101010101010101".into(), // 02 < changes here
            "01010101010101010101010101010101".into(), // 03 < changes here
            "01010101010101010101010101010101".into(), // 04 < changes here
            "01010101010101010101010101010101".into(), // 05 < changes here
            "01010101010101010101010101010101".into(), // 06 < changes here
            "01010101010101010101010101010101".into(), // 07 < changes here
            "01010101010101010101010101010101".into(), // 08 < changes here
            "01010101010101010101010101010101".into(), // 09 < changes here
            "01010101010101010101010101010101".into(), // 10 < changes here
            "01010101010101010101010101010101".into(), // 11 < changes here
            "01010101010101010101010101010101".into(), // 12 < changes here
            "01010101010101010101010101010101".into(), // 13 < changes here
            "01010101010101010101010101010101".into(), // 14 < changes here
            "01010101010101010101010101010101".into(), // 15 < changes here
            "01010101010101010101010101010101".into(), // 16 < changes here
            "01010101010101010101010101010101".into(), // 17 < changes here
            "01010101010101010101010101010101".into(), // 18 < changes here
            "01010101010101010101010101010101".into(), // 19 < changes here
            "01010101010101010101010101010101".into(), // 20 < changes here
            "01010101010101010101010101010101".into(), // 21 < changes here
            "01010101010101010101010101010101".into(), // 22 < changes here
            "01010101010101010101010101010101".into(), // 23 < changes here
            "01010101010101010101010101010101".into(), // 24 < changes here
            "01010101010101010101010101010101".into(), // 25 < changes here
            "01010101010101010101010101010101".into(), // 26 < changes here
            "01010101010101010101010101010101".into(), // 27 < changes here
            "01010101010101010101010101010101".into(), // 28 < changes here
            "01010101010101010101010101010101".into(), // 29 < changes here
            "00000000000001110000000000000000".into(), // 30
            "01101010101010101010101010101010".into(), // 31
        ];

        let mut simulator = crate::ssem::simulator::Simulator::from(initial_store_start);
        simulator.run(29);
        let expected_end_state = crate::ssem::simulator::Simulator::from(expected_store_end);
        assert_eq!(simulator.store, expected_end_state.store);
    }
}
