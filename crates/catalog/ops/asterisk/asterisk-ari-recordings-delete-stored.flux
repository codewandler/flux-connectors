op asterisk-ari-recordings-delete-stored(recordingName: String) -> Any
  description "Delete a stored recording."
  risk "destructive"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/recordings/stored/{recordingName}")
  response = http.request(method: "DELETE", url)
  return response
