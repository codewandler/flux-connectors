op asterisk-ari-bridges-clear-video-source(bridgeId: String) -> Any
  description "Removes any explicit video source in a multi-party mixing bridge. This operation has no effect on bridges with two or fewer participants. When no explicit video source is set, talk detection will be used to determine the active video stream."
  risk "destructive"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/bridges/{bridgeId}/videoSource")
  response = http.request(method: "DELETE", url)
  return response
