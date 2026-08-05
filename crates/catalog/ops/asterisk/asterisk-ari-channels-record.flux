op asterisk-ari-channels-record(channelId: String, name: String, format: String, maxDurationSeconds: Number, maxSilenceSeconds: Number, ifExists: String, beep: Bool, terminateOn: String) -> Any
  description "Start a recording."
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/record")
  response = http.request(method: "POST", query: { beep, format, ifExists, maxDurationSeconds, maxSilenceSeconds, name, terminateOn }, url)
  return response
