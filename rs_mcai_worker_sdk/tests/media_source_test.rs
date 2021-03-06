extern crate mcai_worker_sdk;
#[cfg(feature = "media")]
extern crate stainless_ffmpeg_sys;

#[cfg(feature = "media")]
use std::{
  collections::HashMap,
  ffi::CString,
  sync::{Arc, Mutex},
};

#[cfg(feature = "media")]
use stainless_ffmpeg::{
  check_result, format_context::FormatContext, frame::Frame, order::output::OutputStream,
  order::ParameterValue, packet::Packet, tools, tools::rational::Rational,
  video_encoder::VideoEncoder,
};
#[cfg(feature = "media")]
use stainless_ffmpeg_sys::*;

#[cfg(feature = "media")]
use mcai_worker_sdk::message::media::source::Source;

#[cfg(feature = "media")]
unsafe fn write_header(format_context: &FormatContext) -> Result<(), String> {
  let path = CString::new(format_context.filename.as_str()).unwrap();

  check_result!(avio_open(
    &mut (*format_context.format_context).pb as *mut _,
    path.as_ptr(),
    AVIO_FLAG_WRITE
  ));

  check_result!(avformat_write_header(
    format_context.format_context,
    std::ptr::null_mut()
  ));

  Ok(())
}

#[cfg(feature = "media")]
unsafe fn get_black_frame(pixel_format: &str, width: i32, height: i32) -> Result<Frame, String> {
  let mut av_frame = av_frame_alloc();

  let pix_fmt = av_get_pix_fmt(CString::new(pixel_format).unwrap().into_raw());
  (*av_frame).width = width;
  (*av_frame).height = height;
  (*av_frame).format = pix_fmt as i32;

  let ret_code = av_image_alloc(
    (*av_frame).data.as_mut_ptr(),
    (*av_frame).linesize.as_mut_ptr(),
    (*av_frame).width,
    (*av_frame).height,
    pix_fmt,
    1,
  );
  check_result!(ret_code);

  Ok(Frame {
    name: Some("black_frame".to_string()),
    frame: av_frame,
    index: 0,
  })
}

#[cfg(feature = "media")]
unsafe fn write_frame(
  format_context: &FormatContext,
  video_encoder: &mut VideoEncoder,
  frame: &Frame,
  interleaved: bool,
) -> Result<(), String> {
  let av_packet = av_packet_alloc();
  av_init_packet(av_packet);
  (*av_packet).data = std::ptr::null_mut();
  (*av_packet).size = 0;
  (*av_packet).pts = video_encoder.pts;

  let packet = Packet {
    name: None,
    packet: av_packet,
  };

  if video_encoder.encode(&frame, &packet)? {
    (*av_packet).stream_index = video_encoder.stream_index as i32;

    if interleaved {
      let ret_code = av_interleaved_write_frame(format_context.format_context, av_packet);
      check_result!(ret_code);
    } else {
      let ret_code = av_write_frame(format_context.format_context, av_packet);
      check_result!(ret_code);
    }
  }
  Ok(())
}

#[cfg(feature = "media")]
unsafe fn flush_encoder(
  format_context: &FormatContext,
  video_encoder: &VideoEncoder,
  interleaved: bool,
) -> Result<(), String> {
  let av_packet = av_packet_alloc();
  av_init_packet(av_packet);
  (*av_packet).data = std::ptr::null_mut();
  (*av_packet).size = 0;

  let packet = Packet {
    name: None,
    packet: av_packet,
  };

  let ret = avcodec_send_frame(video_encoder.codec_context, std::ptr::null_mut());
  if ret != 0 && ret != AVERROR_EOF {
    check_result!(ret);
  }

  check_result!(avcodec_receive_packet(
    video_encoder.codec_context,
    packet.packet as *mut _
  ));

  (*av_packet).stream_index = video_encoder.stream_index as i32;

  if interleaved {
    let ret_code = av_interleaved_write_frame(format_context.format_context, av_packet);
    check_result!(ret_code);
  } else {
    let ret_code = av_write_frame(format_context.format_context, av_packet);
    check_result!(ret_code);
  }

  Ok(())
}

#[cfg(feature = "media")]
unsafe fn close_file(format_context: &FormatContext) -> Result<(), String> {
  check_result!(av_write_trailer(format_context.format_context));
  Ok(())
}

#[cfg(feature = "media")]
fn create_xdcam_sample_file(file_path: &str, nb_frames: i32) -> Result<(), String> {
  let xdcam_profile = [
    ("gop_size", ParameterValue::Int64(12)),
    ("max_b_frames", ParameterValue::Int64(2)),
    (
      "frame_rate",
      ParameterValue::Rational(Rational { num: 25, den: 1 }),
    ),
    ("width", ParameterValue::Int64(1920)),
    ("height", ParameterValue::Int64(1080)),
    (
      "pixel_format",
      ParameterValue::String("yuv422p".to_string()),
    ),
    ("bitrate", ParameterValue::Int64(50_000_000)),
  ];

  let mut codec_parameters = HashMap::<String, ParameterValue>::new();
  for (key, value) in &xdcam_profile {
    codec_parameters.insert(key.to_string(), value.clone());
  }

  let output_stream = OutputStream {
    label: Some("video_stream".to_string()),
    codec: "mpeg2video".to_string(),
    parameters: codec_parameters,
  };

  let mut video_encoder = VideoEncoder::new("video".to_string(), 0, &output_stream).unwrap();

  let mut format_context = FormatContext::new(file_path)?;
  let output_parameters = HashMap::<String, ParameterValue>::new();
  format_context.open_output(&output_parameters)?;
  format_context.add_video_stream(&video_encoder)?;

  unsafe {
    let black_frame = get_black_frame("yuv422p", 1920, 1080)?;

    write_header(&format_context)?;

    for _i in 0..nb_frames {
      write_frame(&format_context, &mut video_encoder, &black_frame, false)?;
    }

    let mut flush_result = Ok(());
    while flush_result.is_ok() {
      flush_result = flush_encoder(&format_context, &video_encoder, false);
    }

    close_file(&format_context)?;
  }
  Ok(())
}

#[test]
#[cfg(feature = "media")]
pub fn test_media_source_seek() {
  let file_path = "./test_gop.mxf";
  let nb_frames = 50;

  create_xdcam_sample_file(file_path, nb_frames).unwrap();

  let mut format_context = FormatContext::new(file_path).unwrap();
  format_context.open_input().unwrap();

  let time_base = Source::get_stream_time_base(0, &format_context);
  assert_eq!(Rational { num: 1, den: 25 }, time_base);

  let format_context_ref = Arc::new(Mutex::new(format_context));

  let packet = format_context_ref.lock().unwrap().next_packet().unwrap();
  let pts = unsafe { (*packet.packet).pts };
  assert_eq!(0, pts);

  let frame_index = 7;
  let milliseconds = Source::get_milliseconds_from_pts(frame_index, &time_base);
  assert_eq!(280, milliseconds);

  let result = Source::seek_in_stream_at(
    0,
    milliseconds as i64,
    format_context_ref.clone(),
    AVSEEK_FLAG_ANY | AVSEEK_FLAG_FRAME,
  );
  assert!(result.is_ok());

  let packet = format_context_ref.lock().unwrap().next_packet().unwrap();
  let pts = unsafe { (*packet.packet).pts };
  assert_eq!(7, pts);

  let frame_index = 9;
  let milliseconds = Source::get_milliseconds_from_pts(frame_index, &time_base);
  assert_eq!(360, milliseconds);

  let result = Source::seek_in_stream_at(
    0,
    milliseconds as i64,
    format_context_ref.clone(),
    AVSEEK_FLAG_BACKWARD,
  );
  assert!(result.is_ok());

  let packet = format_context_ref.lock().unwrap().next_packet().unwrap();
  let pts = unsafe { (*packet.packet).pts };
  assert_eq!(0, pts);

  std::fs::remove_file(file_path).unwrap();
}
