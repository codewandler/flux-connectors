op asterisk-ari-channels-dial(channelId: String, caller: String, timeout: Number) -> Any
  description "Dial a created channel."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/dial")
  sep = "?"
  when caller
    url = fmt("{url}{sep}caller={caller}")
    sep = "&"
  when $timeout
    url = fmt("{url}{sep}timeout={timeout}")
  response = http.request(method: "POST", url)
  return response
