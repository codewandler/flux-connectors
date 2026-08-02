op asterisk-ari-playbacks-get(playbackId: String) -> Any
  description "Get a playback's details."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{host}:8089/ari"
  url = fmt("{base}/playbacks/{playbackId}")
  response = http.request(method: "GET", url)
  return response
