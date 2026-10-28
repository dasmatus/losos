# The two models LosOS Cloud's Photos finds and tells faces apart with
# (backend/src/cloud/photos/faces.rs), from OpenCV's model zoo at a pinned
# commit: YuNet, the detector (MIT), and SFace, the recogniser (Apache 2.0).
# tract runs them on the box's CPU; nothing about a face leaves the box.
{ pkgs }:

let
  rev = "48b10fa505aeb51fbd127fc60a5c660040e823b7";
  zoo = path: "https://media.githubusercontent.com/media/opencv/opencv_zoo/${rev}/models/${path}";
in
pkgs.linkFarm "losos-face-models" [
  {
    name = "yunet.onnx";
    path = pkgs.fetchurl {
      url = zoo "face_detection_yunet/face_detection_yunet_2023mar.onnx";
      sha256 = "8f2383e4dd3cfbb4553ea8718107fc0423210dc964f9f4280604804ed2552fa4";
    };
  }
  {
    name = "sface.onnx";
    path = pkgs.fetchurl {
      url = zoo "face_recognition_sface/face_recognition_sface_2021dec.onnx";
      sha256 = "0ba9fbfa01b5270c96627c4ef784da859931e02f04419c829e83484087c34e79";
    };
  }
]
