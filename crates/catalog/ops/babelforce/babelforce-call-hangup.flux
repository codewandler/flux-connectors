op babelforce-call-hangup(id: String) -> Any
  description "Hang up a live call"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  $base = "https://services.babelforce.com"
  $url = fmt("{base}/api/v2/calls/{id}/hangup")
  $response = http.request({ method: "POST", url: $url })
  return $response
