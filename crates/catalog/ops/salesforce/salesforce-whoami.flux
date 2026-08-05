op salesforce-whoami -> Any
  description "Get the authenticated user and org for the current access token — the identity check for a settings page's Test Connection button. Takes no parameters"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{instance}.my.salesforce.com"
  url = fmt("{base}/services/oauth2/userinfo")
  response = http.request(method: "GET", url)
  return response
