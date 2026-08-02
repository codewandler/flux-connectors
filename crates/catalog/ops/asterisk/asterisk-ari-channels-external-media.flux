op asterisk-ari-channels-external-media(channelId: String, app: String, external_host: String, encapsulation: String, transport: String, connection_type: String, format: String, direction: String, data: String, transport_data: String, variables: Any) -> Any
  description "Start an External Media session."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/externalMedia?app={app}&format={format}")
  sep = "&"
  when channelId
    url = fmt("{url}{sep}channelId={channelId}")
    sep = "&"
  when external_host
    url = fmt("{url}{sep}external_host={external_host}")
    sep = "&"
  when encapsulation
    url = fmt("{url}{sep}encapsulation={encapsulation}")
    sep = "&"
  when transport
    url = fmt("{url}{sep}transport={transport}")
    sep = "&"
  when connection_type
    url = fmt("{url}{sep}connection_type={connection_type}")
    sep = "&"
  when direction
    url = fmt("{url}{sep}direction={direction}")
    sep = "&"
  when data
    url = fmt("{url}{sep}data={data}")
    sep = "&"
  when transport_data
    url = fmt("{url}{sep}transport_data={transport_data}")
  content_type = "application/json"
  payload = { variables }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
