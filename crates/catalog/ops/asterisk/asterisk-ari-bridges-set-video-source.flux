op asterisk-ari-bridges-set-video-source(bridgeId: String, channelId: String) -> Any
  description "Set a channel as the video source in a multi-party mixing bridge. This operation has no effect on bridges with two or fewer participants."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/bridges/{bridgeId}/videoSource/{channelId}")
  response = http.request(method: "POST", url)
  return response
