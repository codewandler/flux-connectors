op asterisk-ari-recordings-unmute(recordingName: String) -> Any
  description "Unmute a live recording."
  risk "destructive"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/recordings/live/{recordingName}/mute")
  response = http.request(method: "DELETE", url)
  return response
