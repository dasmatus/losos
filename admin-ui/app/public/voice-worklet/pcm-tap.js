/* Hands the call's audio to the page as plain samples. Runs in the audio
 * thread: one 128-sample block per call of process(), sent on as it is. */
class PcmTap extends AudioWorkletProcessor {
  process(inputs) {
    const channel = inputs[0] && inputs[0][0];
    if (channel && channel.length > 0) {
      const copy = new Float32Array(channel);
      this.port.postMessage(copy, [copy.buffer]);
    }
    return true;
  }
}
registerProcessor("pcm-tap", PcmTap);
