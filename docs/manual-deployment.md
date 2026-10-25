# 선택적 수동 바이너리 배포

이 기능은 설치 절차만 제공합니다. 기존 CI의 바이너리 아티팩트와 GHCR 이미지 전달은 유지하며, push로 운영 배포하지 않습니다. 저장소 변수 `ENABLE_MANUAL_DEPLOY`가 정확히 `true`가 아니면 수동 워크플로의 배포 작업도 건너뜁니다. 이 변경에서는 변수·runner·environment를 등록하거나 운영 서비스를 변경하지 않았습니다.

## 로컬 실행

기존 실행 파일이 있는 신뢰된 사용자 소유 디렉터리에서 사용합니다. `.env`, 데이터베이스, systemd unit, tunnel은 수정하지 않습니다. 적용 시 지정한 사용자 서비스만 재시작합니다. 후보 바이너리는 운영 호스트와 호환되는, 검토된 CI 빌드여야 합니다. DB 마이그레이션을 되돌리는 기능은 없습니다.

```bash
# 기본은 dry-run: 파일/백업/서비스를 변경하지 않음
bash scripts/install-binary.sh ./waynudge /absolute/path/waynudge waynudge.service http://127.0.0.1:8080/health
# 명시적으로 적용: 이전 실행 파일은 TARGET.previous에 보관
bash scripts/install-binary.sh --apply ./waynudge /absolute/path/waynudge waynudge.service http://127.0.0.1:8080/health
# 명시적 롤백: 후보 경로는 무시하며 TARGET.previous를 복원
bash scripts/install-binary.sh --apply --rollback unused /absolute/path/waynudge waynudge.service http://127.0.0.1:8080/health
```

적용 전 `WAYNUDGE_HEALTH_TOKEN` 환경 변수에 인증 토큰을 안전하게 주입합니다. 토큰은 curl의 표준 입력으로만 전달하며 명령 인자나 출력에 넣지 않습니다. dry-run에는 토큰이 필요하지 않습니다.

적용에는 Bash, GNU coreutils, `flock`, `systemctl`, `curl`이 필요합니다. 바이너리 교체는 같은 디렉터리에서 원자적 rename으로 수행하며 동시 실행은 잠금으로 거부합니다. 서비스 재시작 후 서비스 활성 상태와 로컬 HTTP 성공 응답을 최대 10회 확인합니다. 실패하면 실행 파일을 복원하고 서비스를 다시 시작하며, 실패한 배포 명령은 성공으로 처리하지 않습니다. 복원 상태 확인까지 실패하면 `ROLLBACK_FAILED`를 출력하므로 운영자 확인이 필요합니다. SIGKILL·정전 또는 DB 변경에는 자동 복구를 보장하지 않습니다.

## GitHub Actions의 선택적 연결

실제 사용을 결정한 운영자가 별도로 다음을 준비합니다.

- 서비스 소유 사용자로 동작하는 신뢰된 Linux x64 self-hosted runner에 `waynudge-deploy` 라벨을 부여합니다. 공개 PR의 임의 코드를 실행하는 runner와 공유하지 않습니다.
- `waynudge-manual` environment에 필요한 승인 규칙을 설정합니다.
- `WAYNUDGE_BINARY`(절대 경로), `WAYNUDGE_SERVICE`(사용자 unit), `WAYNUDGE_HEALTH_URL`(localhost/127.0.0.1 URL) 변수를 설정합니다. runner에 `gh`, `jq`도 설치합니다.
- environment secret `WAYNUDGE_HEALTH_TOKEN`에 상태 확인용 Bearer 토큰을 저장합니다.
- 준비 완료 후에만 저장소 변수 `ENABLE_MANUAL_DEPLOY=true`를 설정합니다.

`Manual binary deployment`를 main/develop에서 수동 실행합니다. 기본 `dry-run`은 검증된 CI 아티팩트를 임시 디렉터리에 다운로드해 설치 예정 작업을 출력합니다. `install`을 명시하면 적용하고, `rollback`은 현재 보관된 이전 바이너리를 복원합니다. 설치에는 main/develop의 성공한 `CI and delivery` run ID를 입력합니다. PR run은 거부합니다. 운영자가 해당 run의 변경과 바이너리 호환성을 검토해야 합니다. GHCR 이미지를 자동 실행하거나 새 서비스를 구성하지 않습니다.

## 격리 검증

`bash scripts/test-install-binary.sh`는 임시 파일과 가짜 systemctl/curl만 사용합니다. 기본 실행의 무변경, 적용과 백업, 명시적 롤백, 상태 확인 실패와 재시작 실패의 복원, 심볼릭 링크 후보 거부를 검사합니다. 실제 systemd 또는 HTTP 요청은 실행하지 않습니다.
