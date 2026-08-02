op asterisk-ari-bridges-add-channel(bridgeId: String, channel: List<String>, role: String, absorbDTMF: Bool, mute: Bool, inhibitConnectedLineUpdates: Bool) -> Any
  description "Add a channel to a bridge."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/bridges/{bridgeId}/addChannel?channel={channel}")
  sep = "&"
  when role
    url = fmt("{url}{sep}role={role}")
    sep = "&"
  when absorbDTMF
    url = fmt("{url}{sep}absorbDTMF={absorbDTMF}")
    sep = "&"
  when mute
    url = fmt("{url}{sep}mute={mute}")
    sep = "&"
  when inhibitConnectedLineUpdates
    url = fmt("{url}{sep}inhibitConnectedLineUpdates={inhibitConnectedLineUpdates}")
  response = http.request(method: "POST", url)
  return response
