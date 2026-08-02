op asterisk-ari-bridges-play(bridgeId: String, media: List<String>, announcer_format: String, lang: String, offsetms: Number, skipms: Number, playbackId: String) -> Any
  description "Start playback of media on a bridge."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/bridges/{bridgeId}/play?media={media}")
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
    sep = "&"
  when playbackId
    url = fmt("{url}{sep}playbackId={playbackId}")
  response = http.request(method: "POST", url)
  return response
