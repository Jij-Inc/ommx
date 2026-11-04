# OpenTelemetryバックエンドへスパンを送信する

OMMXアプリケーションのスパンをOpenTelemetryのコレクターへ送るための設定です。Notebookやスクリプトで手元の記録を読む方法は{mod}`ommx.tracing`、実験のRunと一緒に保存する方法は{attr}`~ommx.experiment.SealedRun.trace`にあります。

SDKの環境に `opentelemetry-exporter-otlp-proto-grpc` をインストールします。`OTEL_EXPORTER_OTLP_ENDPOINT` にコレクターのgRPCエンドポイントを設定し、認証が必要ならその設定も用意してください。OMMXをimportする前に、プロセスまたはNotebookカーネルの先頭で一度だけproviderを設定します。アプリケーションがすでにproviderを持っている場合は、そのproviderにspan processorを追加します。

```python
from opentelemetry import trace
from opentelemetry.exporter.otlp.proto.grpc.trace_exporter import OTLPSpanExporter
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import BatchSpanProcessor

provider = TracerProvider()
provider.add_span_processor(BatchSpanProcessor(OTLPSpanExporter()))
trace.set_tracer_provider(provider)

from ommx import Instance
from ommx.tracing import capture_trace

with capture_trace("evaluate candidate") as captured:
    Instance.minimize().evaluate({})
print(captured)
provider.shutdown()
```

ローカルの記録にも有効なproviderを使うため、この区間のスパンは手元のツリーと外部バックエンドの両方で確認できます。このスクリプトでは終了時にproviderをshutdownし、残っているスパンを送ります。サービス内で使う場合はproviderを管理する側でshutdownします。
