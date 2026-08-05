op babelforce-integration-api-proxy-get(integrationId: String, uri: String) -> Any
  description "Proxy a request to the provider API"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/integrations/{integrationId}/api/{uri}")
  response = http.request(method: "GET", url)
  return response
