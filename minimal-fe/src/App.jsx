import { useState, useEffect } from 'react'
import BarChart from './BarChart'

function App() {
  const [stories, setStories] = useState([])
  const [domains, setDomains] = useState([])
  const [loading, setLoading] = useState(true)

  // Function to fetch domain data
  const fetchDomains = () => {
    // Use relative URL for API requests to work in both development and production
    // Also add a backup URL that tries the Docker container name if localhost fails
    fetch('http://174.138.45.200:3000/api/top-domains')
      .then(res => {
        if (!res.ok) {
          throw new Error(`HTTP error! Status: ${res.status}`);
        }
        return res.json();
      })
      .then(data => {
        setDomains(data.domains || [])
        setLoading(false)
      })

  }

  // Load domains on component mount
  useEffect(() => {
    fetchDomains()
    const interval = setInterval(fetchDomains, 30000) // Refresh every 30 seconds
    return () => clearInterval(interval)
  }, [])
  return (
    <div>
          {/* Bar Chart Section - Added separately */}
          <div style={{ 
            marginTop: '40px', 
            padding: '20px',
            border: '1px solid #0f0',
            backgroundColor: '#121212'
          }}>
            <h2 style={{ color: '#7fff7f', marginBottom: '20px' }}>Hacker News most referenced domains</h2>
            <div style={{ height: '400px', width: '100%' }}>
              <BarChart data={domains.slice(0, 15)} />
            </div>
          </div>
          
      <h1 style={{ color: '#5bff5b', textShadow: '0 0 5px rgba(0, 255, 0, 0.5)' }}>Hacker News Domains</h1>
      
      {loading ? (
        <p>Loading domain data...</p>
      ) : (
        <div>
          <h2>Top 100 Domains</h2>
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: '10px' }}>
            {domains.map((domain, i) => (
              <div 
                key={i} 
                style={{ 
                  border: '1px solid #0f0', 
                  padding: '10px',
                  background: i === 0 ? '#050' : 'transparent'
                }}
              >
                <div><a href={`https://${domain.domain}`} target="_blank" rel="noopener noreferrer">{domain.domain}</a></div>
                <div>Count: {domain.count}</div>
              </div>
            ))}
          </div>
          

        </div>
      )}
    </div>
  )
}

export default App