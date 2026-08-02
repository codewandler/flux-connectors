op asterisk-ari-channels-start-moh(channelId: String, mohClass: String) -> Any
  description "Play music on hold to a channel."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/moh")
  sep = "?"
  when mohClass
    url = fmt("{url}{sep}mohClass={mohClass}")
  response = http.request(method: "POST", url)
  return response
