op asterisk-ari-channels-rtpstatistics(channelId: String) -> Any
  description "RTP stats on a channel."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/rtp_statistics")
  response = http.request(method: "GET", url)
  return response
