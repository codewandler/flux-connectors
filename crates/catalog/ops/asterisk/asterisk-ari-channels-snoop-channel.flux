op asterisk-ari-channels-snoop-channel(channelId: String, spy: String, whisper: String, app: String, appArgs: String, snoopId: String) -> Any
  description "Start snooping."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/snoop?app={app}")
  sep = "&"
  when spy
    url = fmt("{url}{sep}spy={spy}")
    sep = "&"
  when whisper
    url = fmt("{url}{sep}whisper={whisper}")
    sep = "&"
  when appArgs
    url = fmt("{url}{sep}appArgs={appArgs}")
    sep = "&"
  when snoopId
    url = fmt("{url}{sep}snoopId={snoopId}")
  response = http.request(method: "POST", url)
  return response
