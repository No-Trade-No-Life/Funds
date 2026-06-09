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
  const [fundDescription, setFundDescription] = useState('Main fund');
  const [eventFundId, setEventFundId] = useState('fund/main');
  const [investorName, setInvestorName] = useState('Alice');
  const [deposit, setDeposit] = useState('1000');
  const [selectedFundId, setSelectedFundId] = useState('');
  const [equityValue, setEquityValue] = useState('');
  const [detailInvestorName, setDetailInvestorName] = useState('Alice');
  const [taxRate, setTaxRate] = useState('0.2');
  const [taxThresholdDelta, setTaxThresholdDelta] = useState('0');
  const [referrer, setReferrer] = useState('');
  const [rebateRate, setRebateRate] = useState('0');
  const [taxationKind, setTaxationKind] = useState<'legacy' | 'preserve_fund_assets'>('preserve_fund_assets');
  const [credentialLabel, setCredentialLabel] = useState('Main exchange');
  const [exchange, setExchange] = useState<ExchangeKind>('okx');
  const [payload, setPayload] = useState(defaultPayload('okx'));
  const [message, setMessage] = useState('Loading funds...');

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
    setMessage(records.length === 0 ? 'No funds yet. Create one through the API.' : 'Funds loaded.');
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
      setMessage('Credential payload must be valid JSON.');
      return;
    }

    const response = await fetch('/credentials', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        label: credentialLabel,
        exchange,
        payload: parsedPayload,
      }),
    });

    setMessage(response.ok ? 'Credential registered.' : 'Credential registration failed.');
    await refreshDashboard();
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

    setMessage(response.ok ? 'Fund created.' : 'Fund creation failed.');
    await loadFunds();
  }

  async function appendDeposit() {
    const depositValue = Number(deposit);

    if (!Number.isFinite(depositValue)) {
      setMessage('Deposit must be a number.');
      return;
    }

    const response = await fetch(`/funds/${encodeURIComponent(eventFundId)}/events`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        updated_at: new Date().toISOString(),
        comment: `Deposit from ${investorName}`,
        fund_equity: null,
        order: {
          name: investorName,
          deposit: depositValue,
        },
        investor: null,
        taxation: null,
      }),
    });

    setMessage(response.ok ? 'Deposit event appended.' : 'Deposit event failed.');
    await loadFunds();
  }

  async function appendFundEvent(accountId: string, event: FundEvent) {
    const response = await fetch(`/funds/${encodeURIComponent(accountId)}/events`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(event),
    });

    setMessage(response.ok ? 'Fund event appended.' : 'Fund event failed.');
    await loadFunds();
  }

  async function updateFundEquity() {
    const equity = Number(equityValue);

    if (!selectedFund || !Number.isFinite(equity)) {
      setMessage('Select a fund and enter a valid equity value.');
      return;
    }

    await appendFundEvent(selectedFund.account_id, {
      updated_at: new Date().toISOString(),
      comment: 'Fund equity update',
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
      setMessage('Investor update values must be valid numbers.');
      return;
    }

    await appendFundEvent(selectedFund.account_id, {
      updated_at: new Date().toISOString(),
      comment: `Investor update for ${detailInvestorName}`,
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
      setMessage('Select a fund before applying taxation.');
      return;
    }

    await appendFundEvent(selectedFund.account_id, {
      updated_at: new Date().toISOString(),
      comment: 'Taxation event',
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
        <p className="eyebrow">Fund Operations</p>
        <h1>Funds dashboard</h1>
        <p className="intro">A Vite, TypeScript, and React frontend served through the Rust gateway.</p>
        <div className="actions">
          <a href="/docs">Open API Docs</a>
          <button type="button" onClick={() => void refreshDashboard()}>
            Refresh
          </button>
        </div>
      </section>

      <section className="metrics" aria-label="Fund summary metrics">
        <Metric label="Funds" value={funds.length.toString()} />
        <Metric label="Investors" value={totalInvestors.toString()} />
        <Metric label="Assets" value={formatMoney(totalAssets)} />
        <Metric label="Exchange Equity" value={formatMoney(capitalSummary?.total_equity_usd ?? 0)} />
      </section>

      <section className="panel splitPanel">
        <div>
          <div className="panelHeader compact">
            <h2>Create fund</h2>
            <span>Persisted in SQLite</span>
          </div>
          <label>
            Account ID
            <input value={fundAccountId} onChange={(event) => setFundAccountId(event.target.value)} />
          </label>
          <label>
            Description
            <input value={fundDescription} onChange={(event) => setFundDescription(event.target.value)} />
          </label>
          <button type="button" onClick={() => void createFund()}>
            Create fund
          </button>
        </div>
        <div>
          <div className="panelHeader compact">
            <h2>Add deposit</h2>
            <span>Append event</span>
          </div>
          <label>
            Fund account ID
            <input value={eventFundId} onChange={(event) => setEventFundId(event.target.value)} />
          </label>
          <label>
            Investor
            <input value={investorName} onChange={(event) => setInvestorName(event.target.value)} />
          </label>
          <label>
            Deposit
            <input value={deposit} onChange={(event) => setDeposit(event.target.value)} inputMode="decimal" />
          </label>
          <button type="button" onClick={() => void appendDeposit()}>
            Append deposit
          </button>
        </div>
      </section>

      <section className="panel splitPanel">
        <div>
          <div className="panelHeader compact">
            <h2>Credentials</h2>
            <span>{credentials.length} registered</span>
          </div>
          <label>
            Label
            <input value={credentialLabel} onChange={(event) => setCredentialLabel(event.target.value)} />
          </label>
          <label>
            Exchange
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
            Secret payload JSON
            <textarea value={payload} onChange={(event) => setPayload(event.target.value)} rows={6} />
          </label>
          <button type="button" onClick={() => void registerCredential()}>
            Register credential
          </button>
        </div>
        <div className="credentialList">
          {credentials.map((credential) => (
            <article className="credentialCard" key={credential.id}>
              <strong>{credential.label}</strong>
              <span>{credential.exchange}</span>
              <small>{credential.id}</small>
            </article>
          ))}
        </div>
      </section>

      <section className="panel">
        <div className="panelHeader">
          <h2>Capital summary</h2>
          <span>{capitalSummary ? formatMoney(capitalSummary.total_equity_usd) : 'Not loaded'}</span>
        </div>
        <div className="fundList">
          {capitalSummary?.accounts.map((account) => (
            <article className="fundCard" key={account.credential_id}>
              <div>
                <h3>{account.label}</h3>
                <p>{account.exchange} / {account.status}</p>
              </div>
              <dl>
                <dt>Equity</dt>
                <dd>{formatMoney(account.equity_usd)}</dd>
                <dt>Error</dt>
                <dd>{account.error ?? 'None'}</dd>
              </dl>
            </article>
          ))}
        </div>
      </section>

      <section className="panel">
        <div className="panelHeader">
          <h2>Managed funds</h2>
          <span>{message}</span>
        </div>
        <div className="fundList">
          {funds.map((fund) => (
            <article className="fundCard" key={fund.account_id}>
              <div>
                <h3>{fund.account_id}</h3>
                <p>{fund.description}</p>
              </div>
              <dl>
                <dt>Unit price</dt>
                <dd>{fund.state.summary.unit_price.toFixed(4)}</dd>
                <dt>Total share</dt>
                <dd>{fund.state.summary.total_share.toFixed(2)}</dd>
                <dt>Events</dt>
                <dd>{fund.events.length}</dd>
              </dl>
            </article>
          ))}
        </div>
      </section>

      {selectedFund ? (
        <section className="panel detailPanel">
          <div className="panelHeader">
            <h2>Fund detail</h2>
            <select value={selectedFund.account_id} onChange={(event) => setSelectedFundId(event.target.value)}>
              {funds.map((fund) => (
                <option value={fund.account_id} key={fund.account_id}>{fund.account_id}</option>
              ))}
            </select>
          </div>

          <section className="metrics compactMetrics" aria-label="Selected fund metrics">
            <Metric label="Unit price" value={selectedFund.state.summary.unit_price.toFixed(4)} />
            <Metric label="Total assets" value={formatMoney(selectedFund.state.total_assets)} />
            <Metric label="Total profit" value={formatMoney(selectedFund.state.summary.total_profit)} />
            <Metric label="Total tax" value={formatMoney(selectedFund.state.summary.total_tax)} />
          </section>

          <div className="detailGrid">
            <div className="chartCard">
              <div className="panelHeader compact">
                <h3>Net value curve</h3>
                <span>{selectedFund.events.length} events</span>
              </div>
              <NavChart points={navPoints(selectedFund)} />
            </div>

            <div className="operationStack">
              <div className="operationCard">
                <h3>Update equity</h3>
                <label>
                  Total equity
                  <input value={equityValue} onChange={(event) => setEquityValue(event.target.value)} inputMode="decimal" />
                </label>
                <button type="button" onClick={() => void updateFundEquity()}>Update NAV</button>
              </div>

              <div className="operationCard">
                <h3>Investor settings</h3>
                <label>
                  Investor
                  <input value={detailInvestorName} onChange={(event) => setDetailInvestorName(event.target.value)} />
                </label>
                <label>
                  Tax rate
                  <input value={taxRate} onChange={(event) => setTaxRate(event.target.value)} inputMode="decimal" />
                </label>
                <label>
                  Tax threshold delta
                  <input value={taxThresholdDelta} onChange={(event) => setTaxThresholdDelta(event.target.value)} inputMode="decimal" />
                </label>
                <label>
                  Referrer
                  <input value={referrer} onChange={(event) => setReferrer(event.target.value)} />
                </label>
                <label>
                  Rebate rate
                  <input value={rebateRate} onChange={(event) => setRebateRate(event.target.value)} inputMode="decimal" />
                </label>
                <button type="button" onClick={() => void updateInvestor()}>Update investor</button>
              </div>

              <div className="operationCard">
                <h3>Taxation</h3>
                <label>
                  Mode
                  <select value={taxationKind} onChange={(event) => setTaxationKind(event.target.value as typeof taxationKind)}>
                    <option value="preserve_fund_assets">Preserve fund assets</option>
                    <option value="legacy">Legacy</option>
                  </select>
                </label>
                <button type="button" onClick={() => void applyTaxation()}>Apply taxation</button>
              </div>
            </div>
          </div>

          <div className="investorTable">
            {Object.entries(selectedFund.state.investors).map(([name, investor]) => (
              <article className="investorRow" key={name}>
                <strong>{name}</strong>
                <span>Share {investor.share.toFixed(4)}</span>
                <span>Deposit {formatMoney(investor.deposit)}</span>
                <span>Tax rate {(investor.tax_rate * 100).toFixed(2)}%</span>
                <span>Taxed {formatMoney(investor.taxed)}</span>
              </article>
            ))}
          </div>
        </section>
      ) : null}
    </main>
  );
}

function NavChart({ points }: { points: Array<{ index: number; unitPrice: number }> }) {
  const path = chartPath(points);

  return (
    <svg className="navChart" viewBox="0 0 640 220" role="img" aria-label="Net value curve">
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
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    maximumFractionDigits: 0,
  }).format(value);
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
