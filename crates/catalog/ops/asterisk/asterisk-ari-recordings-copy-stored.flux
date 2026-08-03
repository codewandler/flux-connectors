op asterisk-ari-recordings-copy-stored(recordingName: String, destinationRecordingName: String) -> Any
  description "Copy a stored recording."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/recordings/stored/{recordingName}/copy")
  response = http.request(method: "POST", query: { destinationRecordingName }, url)
  return response
