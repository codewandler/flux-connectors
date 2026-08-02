op asterisk-ari-channels-snoop-channel-with-id(channelId: String, snoopId: String, spy: String, whisper: String, app: String, appArgs: String) -> Any
  description "Start snooping."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/snoop/{snoopId}?app={app}")
  sep = "&"
  when spy
    url = fmt("{url}{sep}spy={spy}")
    sep = "&"
  when whisper
    url = fmt("{url}{sep}whisper={whisper}")
    sep = "&"
  when appArgs
    url = fmt("{url}{sep}appArgs={appArgs}")
  response = http.request(method: "POST", url)
  return response
