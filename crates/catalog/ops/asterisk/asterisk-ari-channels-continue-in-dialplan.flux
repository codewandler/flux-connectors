op asterisk-ari-channels-continue-in-dialplan(channelId: String, context: String, extension: String, priority: Number, label: String) -> Any
  description "Exit application; continue execution in the dialplan."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/continue")
  sep = "?"
  when context
    url = fmt("{url}{sep}context={context}")
    sep = "&"
  when extension
    url = fmt("{url}{sep}extension={extension}")
    sep = "&"
  when priority
    url = fmt("{url}{sep}priority={priority}")
    sep = "&"
  when label
    url = fmt("{url}{sep}label={label}")
  response = http.request(method: "POST", url)
  return response
