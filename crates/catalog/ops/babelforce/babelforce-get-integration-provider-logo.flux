op babelforce-get-integration-provider-logo(providerName: String, size: String) -> Any
  description "Get a provider logo"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/integrations/{providerName}/logo/{size}")
  response = http.request(method: "GET", url)
  return response
