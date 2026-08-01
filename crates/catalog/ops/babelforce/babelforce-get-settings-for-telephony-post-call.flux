op babelforce-get-settings-for-telephony-post-call -> Any
  description "Get post-call settings"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/settings/telephony/post-call")
  response = http.request(method: "GET", url)
  return response
