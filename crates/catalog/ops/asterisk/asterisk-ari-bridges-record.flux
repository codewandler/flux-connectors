op asterisk-ari-bridges-record(bridgeId: String, name: String, format: String, recorder_format: String, maxDurationSeconds: Number, maxSilenceSeconds: Number, ifExists: String, beep: Bool, terminateOn: String) -> Any
  description "Start a recording."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/bridges/{bridgeId}/record")
  response = http.request(method: "POST", query: { beep, format, ifExists, maxDurationSeconds, maxSilenceSeconds, name, recorder_format, terminateOn }, url)
  return response
