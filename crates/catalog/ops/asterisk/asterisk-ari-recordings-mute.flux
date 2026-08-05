op asterisk-ari-recordings-mute(recordingName: String) -> Any
  description "Mute a live recording."
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/recordings/live/{recordingName}/mute")
  response = http.request(method: "POST", url)
  return response
