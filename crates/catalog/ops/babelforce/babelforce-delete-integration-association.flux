op babelforce-delete-integration-association(integrationId: String, associationId: String, actionName: String) -> Any
  description "Remove an integration association"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/integrations/{integrationId}/association/{associationId}/{actionName}")
  response = http.request(method: "DELETE", url)
  return response
