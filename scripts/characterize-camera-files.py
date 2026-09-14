"""Decode saved CM678 USB descriptors and JPEG headers without changing hardware."""
import argparse
import hashlib
import json
import struct
import uuid
from pathlib import Path
from PIL import Image


def descriptors(data):
    position = 0
    while position < len(data):
        length = data[position]
        if length < 2 or position + length > len(data):
            raise ValueError(f"Invalid USB descriptor at {position}")
        yield position, data[position:position+length]
        position += length


def usb_report(root):
    data = (root / "configuration.bin").read_bytes()
    if len(data) != struct.unpack_from("<H", data, 2)[0]:
        raise ValueError("Configuration wTotalLength mismatch")
    connection = (root / "connection.bin").read_bytes()
    result = {"usb_bcd_hex": f"{struct.unpack_from('<H',connection,6)[0]:04x}",
              "device_release_bcd_hex": f"{struct.unpack_from('<H',connection,16)[0]:04x}",
              "configuration_sha256": hashlib.sha256(data).hexdigest(),
              "uvc_headers": [], "stream_headers": [], "formats": [], "extension_units": [],
              "color_descriptors": [], "still_descriptors": [], "camera_terminals": [], "processing_units": []}
    interface = None
    current_format = None
    raw = []
    for offset, d in descriptors(data):
        raw.append(f"{offset:04d}: {d.hex(' ')}")
        if d[1] == 4:
            interface = {"number":d[2],"alternate":d[3],"class":d[5],"subclass":d[6]}
        if d[1] != 0x24 or not interface or interface["class"] != 14:
            continue
        subtype = d[2]
        if interface["subclass"] == 1:
            if subtype == 1:
                result["uvc_headers"].append({"offset":offset,"bcd_uvc_hex":f"{struct.unpack_from('<H',d,3)[0]:04x}"})
            elif subtype == 2 and struct.unpack_from("<H",d,4)[0] == 0x0201:
                mask = int.from_bytes(d[15:15+d[14]], "little")
                result["camera_terminals"].append({"offset":offset,"control_mask_hex":hex(mask),
                    "auto_exposure_mode":bool(mask & (1<<1)), "exposure_absolute":bool(mask & (1<<3)),
                    "focus_absolute":bool(mask & (1<<5)),"focus_auto":bool(mask & (1<<17)),
                    "objective_focal_length_min_raw":struct.unpack_from('<H',d,8)[0],
                    "objective_focal_length_max_raw":struct.unpack_from('<H',d,10)[0],
                    "ocular_focal_length_raw":struct.unpack_from('<H',d,12)[0]})
            elif subtype == 5:
                mask = int.from_bytes(d[8:8+d[7]],"little")
                result["processing_units"].append({"offset":offset,"control_mask_hex":hex(mask),"power_line_frequency_advertised":bool(mask & (1<<10))})
            elif subtype == 6:
                result["extension_units"].append({"offset":offset,"unit_id":d[3],"guid":str(uuid.UUID(bytes_le=d[4:20])),"num_controls":d[20],"raw_hex":d.hex()})
        elif interface["subclass"] == 2:
            if subtype == 1:
                result["stream_headers"].append({"offset":offset,"interface":interface["number"],"still_capture_method":d[9],"trigger_support":d[10],"trigger_usage":d[11]})
            elif subtype in (4,6):
                current_format = {"offset":offset,"index":d[3],"name":"MJPEG" if subtype==6 else "uncompressed", "frames":[]}
                if subtype==4:
                    current_format.update({"guid":str(uuid.UUID(bytes_le=d[5:21])),"fourcc":d[5:9].decode("ascii",errors="replace"),"bits_per_pixel":d[21]})
                result["formats"].append(current_format)
            elif subtype in (5,7) and current_format is not None:
                count = d[25]
                intervals = list(struct.unpack_from("<"+"I"*(count if count else 3), d,26))
                current_format["frames"].append({"width":struct.unpack_from('<H',d,5)[0],"height":struct.unpack_from('<H',d,7)[0],
                    "interval_type":count,"intervals_100ns":intervals,"fps": [1e7/v for v in intervals] if count else None})
            elif subtype == 3:
                count = d[4]
                sizes = [list(struct.unpack_from("<HH",d,5+i*4)) for i in range(count)]
                result["still_descriptors"].append({"offset":offset,"format_index":current_format["index"] if current_format else None,"endpoint":d[3],"sizes":sizes,"compression_pattern_count":d[5+count*4]})
            elif subtype == 13:
                result["color_descriptors"].append({"offset":offset,"preceding_format_index":current_format["index"] if current_format else None,
                    "primaries_code":d[3],"transfer_code":d[4],"matrix_code":d[5],"raw_hex":d.hex(),
                    "note":"UVC Color Matching descriptor values, not Media Foundation enum values. No nominal-range field in this descriptor."})
    (root/"descriptors.txt").write_text("\n".join(raw),encoding="utf-8")
    (root/"decoded.json").write_text(json.dumps(result,indent=2),encoding="utf-8")
    return result


def jpeg_report(root):
    rows = []
    tables = {}
    for path in sorted((root/"native-frames").glob("*.jpg")):
        data = path.read_bytes()
        with Image.open(path) as image:
            if image.format != "JPEG":
                raise ValueError(f"Not JPEG: {path}")
            quantization = image.quantization
            if not quantization:
                raise ValueError(f"No DQT: {path}")
            signature = hashlib.sha256(json.dumps(quantization,sort_keys=True).encode()).hexdigest()
            tables[signature] = {str(key):{"natural_order_8x8":[values[i:i+8] for i in range(0,64,8)],"min":min(values),"max":max(values)} for key,values in quantization.items()}
            rows.append({"file":str(path),"sha256":hashlib.sha256(data).hexdigest(),"width":image.width,"height":image.height,
                "bytes":len(data),"bits_per_pixel":8*len(data)/(image.width*image.height),
                "components_id_h_v_quant_table":image.layer,"dqt_signature":signature,
                "header_metadata_keys":list(image.info)})
    if not rows: raise ValueError("No JPEG samples")
    report = {"samples":rows,"unique_dqt_sets":len(tables),"quantization_tables":tables,
        "mean_bytes":sum(r["bytes"] for r in rows)/len(rows),
        "limitations":["Stream copied from ffmpeg DirectShow MJPEG AVI; no JPEG re-encoding. Not sensor RAW.",
            "DQT is measured. There is no authoritative universal JPEG quality percentage.",
            "Tables describe quantization, not total perceptual loss; no matched YUY2 baseline.",
            "Header metadata is not proof of exact physical color response."]}
    (root/"compression-report.json").write_text(json.dumps(report,indent=2),encoding="utf-8")
    return report


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root",type=Path)
    args=parser.parse_args()
    usb=usb_report(args.root/"usb")
    jpeg=jpeg_report(args.root/"mjpeg")
    print(json.dumps({"uvc":usb["uvc_headers"],"still":usb["stream_headers"],"colors":usb["color_descriptors"],
        "camera_controls":usb["camera_terminals"],"processing_units":usb["processing_units"],
        "jpeg_unique_dqt":jpeg["unique_dqt_sets"],"jpeg_mean_bytes":jpeg["mean_bytes"],
        "jpeg_components":jpeg["samples"][0]["components_id_h_v_quant_table"]},indent=2))


if __name__ == "__main__": main()
