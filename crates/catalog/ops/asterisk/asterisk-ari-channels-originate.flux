op asterisk-ari-channels-originate(endpoint: String, extension: String, context: String, priority: Number, label: String, app: String, appArgs: String, callerId: String, timeout: Number, channelId: String, otherChannelId: String, originator: String, formats: String, variables: Any) -> Any
  description "Create a new channel (originate)."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels?endpoint={endpoint}")
  sep = "&"
  when extension
    url = fmt("{url}{sep}extension={extension}")
    sep = "&"
  when context
    url = fmt("{url}{sep}context={context}")
    sep = "&"
  when priority
    url = fmt("{url}{sep}priority={priority}")
    sep = "&"
  when label
    url = fmt("{url}{sep}label={label}")
    sep = "&"
  when app
    url = fmt("{url}{sep}app={app}")
    sep = "&"
  when appArgs
    url = fmt("{url}{sep}appArgs={appArgs}")
    sep = "&"
  when callerId
    url = fmt("{url}{sep}callerId={callerId}")
    sep = "&"
  when $timeout
    url = fmt("{url}{sep}timeout={timeout}")
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
