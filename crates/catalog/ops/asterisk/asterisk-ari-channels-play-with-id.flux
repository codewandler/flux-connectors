op asterisk-ari-channels-play-with-id(channelId: String, playbackId: String, media: List<String>, lang: String, offsetms: Number, skipms: Number) -> Any
  description "Start playback of media and specify the playbackId."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/play/{playbackId}?media={media}")
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
