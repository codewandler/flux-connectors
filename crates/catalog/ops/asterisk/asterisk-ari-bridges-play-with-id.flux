op asterisk-ari-bridges-play-with-id(bridgeId: String, playbackId: String, media: List<String>, announcer_format: String, lang: String, offsetms: Number, skipms: Number) -> Any
  description "Start playback of media on a bridge."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/bridges/{bridgeId}/play/{playbackId}?media={media}")
  sep = "&"
  when announcer_format
    url = fmt("{url}{sep}announcer_format={announcer_format}")
    sep = "&"
  when lang
    url = fmt("{url}{sep}lang={lang}")
    sep = "&"
  when offsetms
    url = fmt("{url}{sep}offsetms={offsetms}")
    sep = "&"
  when skipms
    url = fmt("{url}{sep}skipms={skipms}")
  response = http.request(method: "POST", url)
  return response
