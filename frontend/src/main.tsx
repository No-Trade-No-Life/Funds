import { StrictMode, useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import './styles.css';

type FundRecord = {
  account_id: string;
  description: string;
  state: {
    total_assets: number;
    summary: {
      total_share: number;
      unit_price: number;
      total_profit: number;
    };
    investors: Record<string, { share: number; deposit: number }>;
  };
  events: unknown[];
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

  const totalAssets = funds.reduce((sum, fund) => sum + fund.state.total_assets, 0);
  const totalInvestors = funds.reduce((sum, fund) => sum + Object.keys(fund.state.investors).length, 0);

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
    </main>
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

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
