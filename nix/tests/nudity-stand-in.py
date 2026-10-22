# A stand-in for the nudity warning's model (src/lib/nudity.ts), for the VM
# tests: the same input and output as Marqo's ViT ("input", 1x3x384x384 in
# [-1, 1]; "logits", [nsfw, safe]), but its "nsfw" logit is only how much
# redder than green the picture is. A plain red square is flagged and a
# plain green one is not, so the tests drive both paths with pictures that
# show nothing at all.
import sys

import onnx
from onnx import TensorProto, helper

weights = helper.make_tensor("w", TensorProto.FLOAT, [3, 2], [20.0, 0.0, -20.0, 0.0, 0.0, 0.0])
graph = helper.make_graph(
    [
        helper.make_node("ReduceMean", ["input"], ["mean"], axes=[2, 3], keepdims=0),
        helper.make_node("MatMul", ["mean", "w"], ["logits"]),
    ],
    "nudity-stand-in",
    [helper.make_tensor_value_info("input", TensorProto.FLOAT, [1, 3, 384, 384])],
    [helper.make_tensor_value_info("logits", TensorProto.FLOAT, [1, 2])],
    [weights],
)
model = helper.make_model(graph, opset_imports=[helper.make_opsetid("", 13)], producer_name="losos-tests")
model.ir_version = 8
onnx.checker.check_model(model)
onnx.save(model, sys.argv[1])
