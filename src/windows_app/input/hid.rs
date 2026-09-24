#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HidButtonDefinition {
    pub(crate) name: &'static str,
    pub(crate) device_name_contains: &'static str,
    pub(crate) report_id: Option<u8>,
    pub(crate) byte_offset: usize,
    pub(crate) bit_mask: u8,
    pub(crate) active_low: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct HidPayload<'a> {
    pub(crate) report_size: u32,
    pub(crate) report_count: u32,
    pub(crate) bytes: &'a [u8],
}

pub(crate) const CUSTOM_BUTTONS: &[HidButtonDefinition] = &[];

pub(crate) fn parse_payload(payload: &[u8]) -> Option<HidPayload<'_>> {
    let header = payload.get(..8)?;
    let report_size = u32::from_ne_bytes(header[0..4].try_into().ok()?);
    let report_count = u32::from_ne_bytes(header[4..8].try_into().ok()?);
    if report_size == 0 || report_count == 0 {
        return None;
    }
    let data_len = report_size.checked_mul(report_count)? as usize;
    let bytes = payload.get(8..8usize.checked_add(data_len)?)?;
    Some(HidPayload {
        report_size,
        report_count,
        bytes,
    })
}

pub(crate) fn decode_buttons<'a>(
    definitions: &'a [HidButtonDefinition],
    device_name: &str,
    report: &[u8],
) -> Vec<(&'a str, bool)> {
    definitions
        .iter()
        .filter(|definition| device_name.contains(definition.device_name_contains))
        .filter(|definition| {
            definition
                .report_id
                .is_none_or(|id| report.first() == Some(&id))
        })
        .filter_map(|definition| {
            let value = report.get(definition.byte_offset)? & definition.bit_mask != 0;
            Some((definition.name, value != definition.active_low))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multiple_reports() {
        let payload = [2, 0, 0, 0, 3, 0, 0, 0, 1, 0x80, 1, 0, 1, 0x40];
        assert_eq!(
            parse_payload(&payload),
            Some(HidPayload {
                report_size: 2,
                report_count: 3,
                bytes: &payload[8..],
            })
        );
    }

    #[test]
    fn rejects_truncated_zero_and_overflowing_payloads() {
        assert_eq!(parse_payload(&[1, 2, 3]), None);
        assert_eq!(parse_payload(&[0, 0, 0, 0, 1, 0, 0, 0]), None);
        assert_eq!(parse_payload(&[1, 0, 0, 0, 0, 0, 0, 0]), None);
        assert_eq!(parse_payload(&[0xff, 0xff, 0xff, 0xff, 2, 0, 0, 0]), None);
        assert_eq!(parse_payload(&[4, 0, 0, 0, 1, 0, 0, 0, 1, 2]), None);
    }

    #[test]
    fn decodes_high_bit_and_active_low_buttons() {
        let definitions = [
            HidButtonDefinition {
                name: "paddle-4",
                device_name_contains: "VID_1234",
                report_id: Some(5),
                byte_offset: 2,
                bit_mask: 0x80,
                active_low: false,
            },
            HidButtonDefinition {
                name: "safety-release",
                device_name_contains: "VID_1234",
                report_id: Some(5),
                byte_offset: 3,
                bit_mask: 0x04,
                active_low: true,
            },
        ];

        assert_eq!(
            decode_buttons(&definitions, r"\\?\HID#VID_1234&PID_5678", &[5, 0, 0x80, 0]),
            vec![("paddle-4", true), ("safety-release", true)]
        );
    }

    #[test]
    fn filters_device_report_id_and_short_reports() {
        let definitions = [HidButtonDefinition {
            name: "auxiliary",
            device_name_contains: "VID_ABCD&PID_0001",
            report_id: Some(9),
            byte_offset: 4,
            bit_mask: 0x02,
            active_low: false,
        }];

        assert!(decode_buttons(&definitions, "VID_OTHER", &[9, 0, 0, 0, 2]).is_empty());
        assert!(decode_buttons(&definitions, "VID_ABCD&PID_0001", &[8, 0, 0, 0, 2]).is_empty());
        assert!(decode_buttons(&definitions, "VID_ABCD&PID_0001", &[9, 0]).is_empty());
    }
}
