op asterisk-ari-channels-create(endpoint: String, app: String, appArgs: String, channelId: String, otherChannelId: String, originator: String, formats: String, variables: Any) -> Any
  description "Create channel."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/create?endpoint={endpoint}&app={app}")
  sep = "&"
  when appArgs
    url = fmt("{url}{sep}appArgs={appArgs}")
    sep = "&"
  when channelId
    url = fmt("{url}{sep}channelId={channelId}")
    sep = "&"
  when otherChannelId
    url = fmt("{url}{sep}otherChannelId={otherChannelId}")
    sep = "&"
  when originator
    url = fmt("{url}{sep}originator={originator}")
    sep = "&"
  when formats
    url = fmt("{url}{sep}formats={formats}")
  content_type = "application/json"
  payload = { variables }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
