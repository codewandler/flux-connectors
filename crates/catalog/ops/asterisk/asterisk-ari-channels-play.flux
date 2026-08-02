op asterisk-ari-channels-play(channelId: String, media: List<String>, lang: String, offsetms: Number, skipms: Number, playbackId: String) -> Any
  description "Start playback of media."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/play?media={media}")
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
