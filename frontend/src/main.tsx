import { StrictMode, useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import './styles.css';

type FundRecord = {
  account_id: string;
  description: string;
  state: {
    total_assets: number;
    total_taxed: number;
    summary: {
      total_deposit: number;
      total_share: number;
      total_tax: number;
      unit_price: number;
      total_profit: number;
    };
    investors: Record<string, InvestorMeta>;
    investor_derived: Record<string, InvestorDerived>;
  };
  events: FundEvent[];
};

type InvestorMeta = {
  share: number;
  tax_threshold: number;
  deposit: number;
  tax_rate: number;
  avg_cost_price: number;
  referrer: string | null;
  referrer_rebate_rate: number;
  claimed_referrer_rebate: number;
  taxed: number;
};

type InvestorDerived = {
  pre_tax_assets: number;
  taxable: number;
  tax: number;
  after_tax_assets: number;
  after_tax_profit: number;
  after_tax_share: number;
  floating_profit_rate: number;
  share_ratio: number;
};

type FundEvent = {
  updated_at: string;
  comment: string | null;
  fund_equity: { equity: number } | null;
  order: { name: string; deposit: number } | null;
  investor: {
    name: string;
    tax_rate: number | null;
    add_tax_threshold: number | null;
    referrer: string | null;
    referrer_rebate_rate: number | null;
  } | null;
  taxation: 'legacy' | 'preserve_fund_assets' | null;
};

type ExchangeKind = 'okx' | 'gate' | 'binance' | 'aster' | 'hyperliquid' | 'bitget' | 'htx';

type CredentialView = {
  id: string;
  label: string;
  exchange: ExchangeKind;
  created_at: string;
};

type CapitalSummary = {
  total_equity_usd: number;
  accounts: {
    credential_id: string;
    label: string;
    exchange: ExchangeKind;
    status: 'ok' | 'failed';
    equity_usd: number;
    error: string | null;
  }[];
};

function App() {
  const [funds, setFunds] = useState<FundRecord[]>([]);
  const [credentials, setCredentials] = useState<CredentialView[]>([]);
  const [capitalSummary, setCapitalSummary] = useState<CapitalSummary | null>(null);
  const [fundAccountId, setFundAccountId] = useState('fund/main');
  const [fundDescription, setFundDescription] = useState('主基金');
  const [eventFundId, setEventFundId] = useState('fund/main');
  const [investorName, setInvestorName] = useState('张三');
  const [deposit, setDeposit] = useState('1000');
  const [selectedFundId, setSelectedFundId] = useState('');
  const [equityValue, setEquityValue] = useState('');
  const [detailInvestorName, setDetailInvestorName] = useState('张三');
  const [taxRate, setTaxRate] = useState('0.2');
  const [taxThresholdDelta, setTaxThresholdDelta] = useState('0');
  const [referrer, setReferrer] = useState('');
  const [rebateRate, setRebateRate] = useState('0');
  const [taxationKind, setTaxationKind] = useState<'legacy' | 'preserve_fund_assets'>('preserve_fund_assets');
  const [credentialLabel, setCredentialLabel] = useState('主交易所账户');
  const [exchange, setExchange] = useState<ExchangeKind>('okx');
  const [payload, setPayload] = useState(defaultPayload('okx'));
  const [message, setMessage] = useState('正在加载基金...');
  const [credentialMessage, setCredentialMessage] = useState('尚未操作凭证。');

  useEffect(() => {
    void refreshDashboard();
  }, []);

  async function refreshDashboard() {
    await Promise.all([loadFunds(), loadCredentials(), loadCapitalSummary()]);
  }

  async function loadFunds() {
    const response = await fetch('/funds');
    const records = (await response.json()) as FundRecord[];
    setFunds(records);
    setSelectedFundId((current) => current || records[0]?.account_id || '');
    setMessage(records.length === 0 ? '暂无基金，请先创建基金。' : '基金已加载。');
  }

  async function loadCredentials() {
    const response = await fetch('/credentials');
    setCredentials((await response.json()) as CredentialView[]);
  }

  async function loadCapitalSummary() {
    const response = await fetch('/capital-summary');
    setCapitalSummary((await response.json()) as CapitalSummary);
  }

  async function registerCredential() {
    let parsedPayload: unknown;

    try {
      parsedPayload = JSON.parse(payload);
    } catch {
      setCredentialMessage('凭证密钥内容必须是有效 JSON。');
      return;
    }

    setCredentialMessage('正在注册凭证...');

    const response = await fetch('/credentials', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        label: credentialLabel,
        exchange,
        payload: parsedPayload,
      }),
    });

    await loadCredentials();
    setCredentialMessage(response.ok ? '凭证已注册。' : '凭证注册失败。');
  }

  async function deleteCredential(id: string) {
    setCredentialMessage('正在删除凭证...');

    const response = await fetch(`/credentials/${encodeURIComponent(id)}`, {
      method: 'DELETE',
    });

    await loadCredentials();
    setCredentialMessage(response.ok ? '凭证已删除。' : '凭证删除失败。');
  }

  async function createFund() {
    const response = await fetch('/funds', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        account_id: fundAccountId,
        description: fundDescription,
      }),
    });

    setMessage(response.ok ? '基金已创建。' : '基金创建失败。');
    await loadFunds();
  }

  async function appendDeposit() {
    const depositValue = Number(deposit);

    if (!Number.isFinite(depositValue)) {
      setMessage('入金金额必须是数字。');
      return;
    }

    const response = await fetch(`/funds/${encodeURIComponent(eventFundId)}/events`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        updated_at: new Date().toISOString(),
        comment: `${investorName} 入金`,
        fund_equity: null,
        order: {
          name: investorName,
          deposit: depositValue,
        },
        investor: null,
        taxation: null,
      }),
    });

    setMessage(response.ok ? '入金事件已追加。' : '入金事件追加失败。');
    await loadFunds();
  }

  async function appendFundEvent(accountId: string, event: FundEvent) {
    const response = await fetch(`/funds/${encodeURIComponent(accountId)}/events`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(event),
    });

    setMessage(response.ok ? '基金事件已追加。' : '基金事件追加失败。');
    await loadFunds();
  }

  async function updateFundEquity() {
    const equity = Number(equityValue);

    if (!selectedFund || !Number.isFinite(equity)) {
      setMessage('请选择基金并输入有效总权益。');
      return;
    }

    await appendFundEvent(selectedFund.account_id, {
      updated_at: new Date().toISOString(),
      comment: '更新基金总权益',
      fund_equity: { equity },
      order: null,
      investor: null,
      taxation: null,
    });
  }

  async function updateInvestor() {
    const parsedTaxRate = Number(taxRate);
    const parsedThresholdDelta = Number(taxThresholdDelta);
    const parsedRebateRate = Number(rebateRate);

    if (!selectedFund || !Number.isFinite(parsedTaxRate) || !Number.isFinite(parsedThresholdDelta) || !Number.isFinite(parsedRebateRate)) {
      setMessage('投资人设置中的数值必须是有效数字。');
      return;
    }

    await appendFundEvent(selectedFund.account_id, {
      updated_at: new Date().toISOString(),
      comment: `更新 ${detailInvestorName} 的投资人设置`,
      fund_equity: null,
      order: null,
      investor: {
        name: detailInvestorName,
        tax_rate: parsedTaxRate,
        add_tax_threshold: parsedThresholdDelta,
        referrer: referrer || null,
        referrer_rebate_rate: parsedRebateRate,
      },
      taxation: null,
    });
  }

  async function applyTaxation() {
    if (!selectedFund) {
      setMessage('请先选择基金再执行结税。');
      return;
    }

    await appendFundEvent(selectedFund.account_id, {
      updated_at: new Date().toISOString(),
      comment: '执行结税',
      fund_equity: null,
      order: null,
      investor: null,
      taxation: taxationKind,
    });
  }

  const totalAssets = funds.reduce((sum, fund) => sum + fund.state.total_assets, 0);
  const totalInvestors = funds.reduce((sum, fund) => sum + Object.keys(fund.state.investors).length, 0);
  const selectedFund = funds.find((fund) => fund.account_id === selectedFundId) ?? funds[0];

  return (
    <main className="shell">
      <section className="hero">
        <p className="eyebrow">基金运营</p>
        <h1>基金管理台</h1>
        <p className="intro">统一管理基金、投资人、交易所凭证和资金汇总。</p>
        <div className="actions">
          <a href="/docs">打开 API 文档</a>
          <button type="button" onClick={() => void refreshDashboard()}>
            刷新
          </button>
        </div>
      </section>

      <section className="metrics" aria-label="基金汇总指标">
        <Metric label="基金数" value={funds.length.toString()} />
        <Metric label="投资人数" value={totalInvestors.toString()} />
        <Metric label="基金资产" value={formatMoney(totalAssets)} />
        <Metric label="交易所权益" value={formatMoney(capitalSummary?.total_equity_usd ?? 0)} />
      </section>

      <section className="panel splitPanel">
        <div>
          <div className="panelHeader compact">
            <h2>创建基金</h2>
            <span>数据持久化到 SQLite</span>
          </div>
          <label>
            基金账户 ID
            <input value={fundAccountId} onChange={(event) => setFundAccountId(event.target.value)} />
          </label>
          <label>
            描述
            <input value={fundDescription} onChange={(event) => setFundDescription(event.target.value)} />
          </label>
          <button type="button" onClick={() => void createFund()}>
            创建基金
          </button>
        </div>
        <div>
          <div className="panelHeader compact">
            <h2>添加入金</h2>
            <span>追加基金事件</span>
          </div>
          <label>
            基金账户 ID
            <input value={eventFundId} onChange={(event) => setEventFundId(event.target.value)} />
          </label>
          <label>
            投资人
            <input value={investorName} onChange={(event) => setInvestorName(event.target.value)} />
          </label>
          <label>
            入金金额
            <input value={deposit} onChange={(event) => setDeposit(event.target.value)} inputMode="decimal" />
          </label>
          <button type="button" onClick={() => void appendDeposit()}>
            追加入金
          </button>
        </div>
      </section>

      <section className="panel splitPanel">
        <div>
          <div className="panelHeader compact">
            <h2>交易所凭证</h2>
            <span>{credentialMessage} 已注册 {credentials.length} 个。</span>
          </div>
          <label>
            名称
            <input value={credentialLabel} onChange={(event) => setCredentialLabel(event.target.value)} />
          </label>
          <label>
            交易所
            <select
              value={exchange}
              onChange={(event) => {
                const nextExchange = event.target.value as ExchangeKind;
                setExchange(nextExchange);
                setPayload(defaultPayload(nextExchange));
              }}
            >
              <option value="okx">OKX</option>
              <option value="gate">Gate</option>
              <option value="binance">Binance</option>
              <option value="aster">Aster</option>
              <option value="hyperliquid">Hyperliquid</option>
              <option value="bitget">Bitget</option>
              <option value="htx">HTX</option>
            </select>
          </label>
          <label>
            密钥 JSON
            <textarea value={payload} onChange={(event) => setPayload(event.target.value)} rows={6} />
          </label>
          <button type="button" onClick={() => void registerCredential()}>
            注册凭证
          </button>
        </div>
        <div className="credentialList">
          {credentials.map((credential) => (
            <article className="credentialCard" key={credential.id}>
              <div>
                <strong>{credential.label}</strong>
                <span>{credential.exchange}</span>
                <small>{credential.id}</small>
              </div>
              <button type="button" onClick={() => void deleteCredential(credential.id)}>删除</button>
            </article>
          ))}
        </div>
      </section>

      <section className="panel">
        <div className="panelHeader">
          <h2>资金汇总</h2>
          <span>{capitalSummary ? formatMoney(capitalSummary.total_equity_usd) : '未加载'}</span>
        </div>
        <div className="fundList">
          {capitalSummary?.accounts.map((account) => (
            <article className="fundCard" key={account.credential_id}>
              <div>
                <h3>{account.label}</h3>
                <p>{account.exchange} / {summaryStatusLabel(account.status)}</p>
              </div>
              <dl>
                <dt>权益</dt>
                <dd>{formatMoney(account.equity_usd)}</dd>
                <dt>错误</dt>
                <dd>{account.error ?? '无'}</dd>
              </dl>
            </article>
          ))}
        </div>
      </section>

      <section className="panel">
        <div className="panelHeader">
          <h2>已管理基金</h2>
          <span>{message}</span>
        </div>
        <div className="fundList">
          {funds.map((fund) => (
            <button
              className={fund.account_id === selectedFund?.account_id ? 'fundCard selectedFundCard' : 'fundCard'}
              key={fund.account_id}
              onClick={() => setSelectedFundId(fund.account_id)}
              type="button"
            >
              <div>
                <h3>{fund.account_id}</h3>
                <p>{fund.description}</p>
              </div>
              <dl>
                <dt>单位净值</dt>
                <dd>{fund.state.summary.unit_price.toFixed(4)}</dd>
                <dt>总份额</dt>
                <dd>{fund.state.summary.total_share.toFixed(2)}</dd>
                <dt>事件数</dt>
                <dd>{fund.events.length}</dd>
              </dl>
            </button>
          ))}
        </div>
      </section>

      {selectedFund ? (
        <section className="panel detailPanel">
          <div className="panelHeader">
            <h2>基金详情</h2>
            <select value={selectedFund.account_id} onChange={(event) => setSelectedFundId(event.target.value)}>
              {funds.map((fund) => (
                <option value={fund.account_id} key={fund.account_id}>{fund.account_id}</option>
              ))}
            </select>
          </div>

          <section className="metrics compactMetrics" aria-label="已选择基金指标">
            <Metric label="单位净值" value={selectedFund.state.summary.unit_price.toFixed(4)} />
            <Metric label="总资产" value={formatMoney(selectedFund.state.total_assets)} />
            <Metric label="总收益" value={formatMoney(selectedFund.state.summary.total_profit)} />
            <Metric label="总税费" value={formatMoney(selectedFund.state.summary.total_tax)} />
          </section>

          <div className="detailGrid">
            <div className="chartCard">
              <div className="panelHeader compact">
                <h3>净值曲线</h3>
                <span>{selectedFund.events.length} 个事件</span>
              </div>
              <NavChart points={navPoints(selectedFund)} />
            </div>

            <div className="operationStack">
              <div className="operationCard">
                <h3>更新权益</h3>
                <label>
                  总权益
                  <input value={equityValue} onChange={(event) => setEquityValue(event.target.value)} inputMode="decimal" />
                </label>
                <button type="button" onClick={() => void updateFundEquity()}>更新净值</button>
              </div>

              <div className="operationCard">
                <h3>投资人设置</h3>
                <label>
                  投资人
                  <input value={detailInvestorName} onChange={(event) => setDetailInvestorName(event.target.value)} />
                </label>
                <label>
                  税率
                  <input value={taxRate} onChange={(event) => setTaxRate(event.target.value)} inputMode="decimal" />
                </label>
                <label>
                  起征点增量
                  <input value={taxThresholdDelta} onChange={(event) => setTaxThresholdDelta(event.target.value)} inputMode="decimal" />
                </label>
                <label>
                  推荐人
                  <input value={referrer} onChange={(event) => setReferrer(event.target.value)} />
                </label>
                <label>
                  返佣比例
                  <input value={rebateRate} onChange={(event) => setRebateRate(event.target.value)} inputMode="decimal" />
                </label>
                <button type="button" onClick={() => void updateInvestor()}>更新投资人</button>
              </div>

              <div className="operationCard">
                <h3>结税</h3>
                <label>
                  模式
                  <select value={taxationKind} onChange={(event) => setTaxationKind(event.target.value as typeof taxationKind)}>
                    <option value="preserve_fund_assets">保留基金资产</option>
                    <option value="legacy">传统模式</option>
                  </select>
                </label>
                <button type="button" onClick={() => void applyTaxation()}>执行结税</button>
              </div>
            </div>
          </div>

          <div className="investorTable">
            {Object.entries(selectedFund.state.investors).map(([name, investor]) => {
              const derived = selectedFund.state.investor_derived[name];

              return (
                <article className="investorRow" key={name}>
                  <strong>{name}</strong>
                  <span>份额 {investor.share.toFixed(4)}</span>
                  <span>入金 {formatMoney(investor.deposit)}</span>
                  <span>税前资产 {formatMoney(derived?.pre_tax_assets ?? investor.share * selectedFund.state.summary.unit_price)}</span>
                  <span>应税额 {formatMoney(derived?.taxable ?? 0)}</span>
                  <span>应缴税 {formatMoney(derived?.tax ?? 0)}</span>
                  <span>税后资产 {formatMoney(derived?.after_tax_assets ?? investor.share * selectedFund.state.summary.unit_price)}</span>
                  <span>占比 {formatPercent(derived?.share_ratio ?? 0)}</span>
                </article>
              );
            })}
          </div>

          <EventTimeline events={selectedFund.events} />
        </section>
      ) : null}
    </main>
  );
}

function EventTimeline({ events }: { events: FundEvent[] }) {
  return (
    <section className="eventTimeline" aria-label="基金事件流水">
      <div className="panelHeader compact">
        <h3>事件流水</h3>
        <span>共 {events.length} 条</span>
      </div>
      <div className="eventList">
        {[...events].reverse().map((event, index) => (
          <article className="eventRow" key={`${event.updated_at}-${index}`}>
            <div>
              <strong>{eventTitle(event)}</strong>
              <p>{event.comment ?? '无备注'}</p>
            </div>
            <span>{formatDateTime(event.updated_at)}</span>
          </article>
        ))}
      </div>
    </section>
  );
}

function eventTitle(event: FundEvent) {
  if (event.order) {
    return `${event.order.name} 入金 ${formatMoney(event.order.deposit)}`;
  }

  if (event.fund_equity) {
    return `总权益更新为 ${formatMoney(event.fund_equity.equity)}`;
  }

  if (event.investor) {
    return `更新 ${event.investor.name} 的投资人设置`;
  }

  if (event.taxation) {
    return `结税：${taxationLabel(event.taxation)}`;
  }

  return '基金事件';
}

function taxationLabel(value: NonNullable<FundEvent['taxation']>) {
  return value === 'preserve_fund_assets' ? '保留基金资产' : '传统模式';
}

function NavChart({ points }: { points: Array<{ index: number; unitPrice: number }> }) {
  const path = chartPath(points);

  return (
    <svg className="navChart" viewBox="0 0 640 220" role="img" aria-label="净值曲线">
      <path className="chartGrid" d="M20 40 H620 M20 110 H620 M20 180 H620" />
      <path className="chartLine" d={path} />
      <text x="24" y="32">{points.at(-1)?.unitPrice.toFixed(4) ?? '1.0000'}</text>
      <text x="24" y="204">1.0000</text>
    </svg>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <article className="metric">
      <span>{label}</span>
      <strong>{value}</strong>
    </article>
  );
}

function formatMoney(value: number) {
  return new Intl.NumberFormat('zh-CN', {
    style: 'currency',
    currency: 'USD',
    maximumFractionDigits: 0,
  }).format(value);
}

function summaryStatusLabel(value: CapitalSummary['accounts'][number]['status']) {
  return value === 'ok' ? '正常' : '失败';
}

function formatPercent(value: number) {
  return `${(value * 100).toFixed(2)}%`;
}

function formatDateTime(value: string) {
  return new Intl.DateTimeFormat('zh-CN', {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(value));
}

function defaultPayload(exchange: ExchangeKind) {
  const payloadByExchange: Record<ExchangeKind, unknown> = {
    okx: { access_key: '', secret_key: '', passphrase: '' },
    gate: { access_key: '', secret_key: '' },
    binance: { access_key: '', secret_key: '' },
    aster: { api_key: '', secret_key: '' },
    hyperliquid: { address: '' },
    bitget: { access_key: '', secret_key: '', passphrase: '' },
    htx: { access_key: '', secret_key: '' },
  };

  return JSON.stringify(payloadByExchange[exchange], null, 2);
}

function navPoints(fund: FundRecord) {
  let totalAssets = 0;
  let totalShare = 0;
  let unitPrice = 1;
  const points = [{ index: 0, unitPrice }];

  fund.events.forEach((event, index) => {
    if (event.fund_equity) {
      totalAssets = event.fund_equity.equity;
    }

    if (event.order) {
      const share = event.order.deposit / unitPrice;
      totalShare += share;
      totalAssets += event.order.deposit;
    }

    unitPrice = totalShare === 0 ? 1 : totalAssets / totalShare;
    points.push({ index: index + 1, unitPrice });
  });

  return points;
}

function chartPath(points: Array<{ index: number; unitPrice: number }>) {
  const maxPrice = Math.max(1, ...points.map((point) => point.unitPrice));
  const width = 600;
  const height = 160;
  const lastIndex = Math.max(1, points.at(-1)?.index ?? 1);

  return points
    .map((point, index) => {
      const x = 20 + (point.index / lastIndex) * width;
      const y = 190 - (point.unitPrice / maxPrice) * height;
      return `${index === 0 ? 'M' : 'L'} ${x.toFixed(2)} ${y.toFixed(2)}`;
    })
    .join(' ');
}

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
