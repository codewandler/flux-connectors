op asterisk-ari-recordings-get-stored-file(recordingName: String) -> Any
  description "Get the file associated with the stored recording."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/recordings/stored/{recordingName}/file")
  response = http.request(method: "GET", url)
  return response
