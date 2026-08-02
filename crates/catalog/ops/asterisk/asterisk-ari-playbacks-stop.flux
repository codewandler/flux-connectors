op asterisk-ari-playbacks-stop(playbackId: String) -> Any
  description "Stop a playback."
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://{host}:8089/ari"
  url = fmt("{base}/playbacks/{playbackId}")
  response = http.request(method: "DELETE", url)
  return response
