op asterisk-ari-channels-send-dtmf(channelId: String, dtmf: String, before: Number, between: Number, duration: Number, after: Number) -> Any
  description "Send provided DTMF to a given channel."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/dtmf")
  sep = "?"
  when dtmf
    url = fmt("{url}{sep}dtmf={dtmf}")
    sep = "&"
  when before
    url = fmt("{url}{sep}before={before}")
    sep = "&"
  when between
    url = fmt("{url}{sep}between={between}")
    sep = "&"
  when duration
    url = fmt("{url}{sep}duration={duration}")
    sep = "&"
  when after
    url = fmt("{url}{sep}after={after}")
  response = http.request(method: "POST", url)
  return response
