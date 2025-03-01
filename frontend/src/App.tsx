/* eslint-disable @typescript-eslint/no-unused-vars */
import { useEffect, useState } from 'react';

// TypeScript interfaces for type safety
interface StoryInfo {
  title: string;
  url?: string;
  author: string;
  score: number;
  comments: number;
  domain?: string;
}

interface HourlyData {
  hour: string;
  stories: StoryInfo[];
  avg_score: number;
  total_comments: number;
  top_authors: [string, number][];
  domains: [string, number][];
}

interface HistoricalData {
  hours: HourlyData[];
  last_update?: string;
}

// SVG Clock component
const ClockIcon = ({ size = 24, className = "", fill = "currentColor" }) => (
  <svg width={size} height={size} viewBox="0 0 8 8" className={className} xmlns="http://www.w3.org/2000/svg">
    <g fill={fill}>
      <rect x="2" y="0" width="4" height="1" />
      <rect x="1" y="1" width="1" height="1" />
      <rect x="6" y="1" width="1" height="1" />
      <rect x="0" y="2" width="1" height="4" />
      <rect x="7" y="2" width="1" height="4" />
      <rect x="1" y="6" width="1" height="1" />
      <rect x="6" y="6" width="1" height="1" />
      <rect x="2" y="7" width="4" height="1" />
      <rect x="3" y="1" width="2" height="4" />
    </g>
  </svg>
);

function App() {
  const [latestLoading, setLatestLoading] = useState(true);
  const [historyLoading, setHistoryLoading] = useState(true);
  const [sourcesLoading, setSourcesLoading] = useState(true);
  const [kafkaData, setKafkaData] = useState<string | null>(null);
  const [historicalData, setHistoricalData] = useState<HistoricalData | null>(null);
  const [sourceCounts, setSourceCounts] = useState<[string, number][]>([]);

  const API_BASE = 'http://100.69.8.70:3000';

  // Fetch latest data only
  const fetchLatestData = async () => {
    try {
      const kafkaResponse = await fetch(`${API_BASE}/api/kafka/latest`);
      if (kafkaResponse.ok) {
        const kafkaText = await kafkaResponse.text();
        setKafkaData(kafkaText);
      }
    } catch (error) {
      console.error('Error fetching latest data:', error);
    } finally {
      setLatestLoading(false);
    }
  };

  // Fetch historical data separately
  const fetchHistoricalData = async () => {
    try {
      const historyResponse = await fetch(`${API_BASE}/api/history`);
      if (historyResponse.ok) {
        const historyData = await historyResponse.json();
        setHistoricalData(historyData);
      }
    } catch (error) {
      console.error('Error fetching historical data:', error);
    } finally {
      setHistoryLoading(false);
    }
  };

  // Fetch source counts data
  const fetchSourceCounts = async () => {
    try {
      const sourcesResponse = await fetch(`${API_BASE}/api/sources`);
      if (sourcesResponse.ok) {
        const sourcesData = await sourcesResponse.json();
        setSourceCounts(sourcesData);
      }
    } catch (error) {
      console.error('Error fetching source counts:', error);
    } finally {
      setSourcesLoading(false);
    }
  };

  // Effect for latest data - runs immediately and every 5 minutes
  useEffect(() => {
    fetchLatestData();
    const latestInterval = setInterval(fetchLatestData, 300000); // 5 minutes
    return () => clearInterval(latestInterval);
  }, []);

  // Effect for historical data - runs after latest data is loaded
  useEffect(() => {
    if (!latestLoading) {
      fetchHistoricalData();
      const historyInterval = setInterval(fetchHistoricalData, 600000); // 10 minutes
      return () => clearInterval(historyInterval);
    }
  }, [latestLoading]);
  
  // Effect for source counts - runs after latest data is loaded
  useEffect(() => {
    if (!latestLoading) {
      fetchSourceCounts();
      const sourcesInterval = setInterval(fetchSourceCounts, 300000); // 5 minutes
      return () => clearInterval(sourcesInterval);
    }
  }, [latestLoading]);

  // Story card component to avoid repetition
  const StoryCard = ({ story }: { story: StoryInfo }) => (
    <div className="py-2 border-b border-green-900 hover:bg-green-900 hover:bg-opacity-20">
      <div className="font-bold">
        <a 
          href={story.url} 
          target="_blank" 
          rel="noopener noreferrer" 
          className="text-green-400 hover:text-green-200"
        >
          &gt;&gt; {story.title}
        </a>
      </div>
      <div className="text-xs text-green-600 mt-1">
        [SCORE: {story.score}] [COMMENTS: {story.comments}] [AGENT: {story.author}]
        {story.domain && ` [SOURCE: ${story.domain}]`}
      </div>
    </div>
  );

  // Hour timestamp display component
  const HourTimestamp = ({ timestamp }: { timestamp: string }) => (
    <div className="mb-2 text-green-300 flex items-center">
      <ClockIcon size={16} className="mr-2" fill="#00ff00" />
      &gt; TIMESTAMP: {new Date(timestamp).toLocaleString()}
    </div>
  );

  return (
    <div 
      className="min-h-screen bg-black text-green-500" 
      style={{ fontFamily: 'monospace' }}
    >
      {/* Ultra-minimalist sources widget - Fixed Position */}
      {!sourcesLoading && sourceCounts.length > 0 && (
        <div className="fixed top-2 right-4 z-50 text-green-500 text-xs border border-green-700 bg-black px-2 py-1">
          <span className="font-bold">TOP:</span> {sourceCounts[0][0]} [{sourceCounts[0][1]}]
        </div>
      )}
      
      <div className="container mx-auto p-4">
        {/* Header */}
        <div className="text-center mb-6">
          <h1 
            className="text-3xl font-bold mb-2 text-green-400 animate-pulse" 
            style={{ textShadow: '0 0 10px #00ff00, 0 0 20px #00ff00' }}
          >
            H4CK3R N3WS TR4CK3R
          </h1>
          
          <div className="flex justify-center space-x-4 my-4">
            <div className="text-green-400">
              <ClockIcon size={32} />
            </div>
            <div className="text-green-300">
              <ClockIcon size={32} fill="#6EE76E" />
            </div>
          </div>
          
          <div className="text-xs mb-4 text-green-300">
            &lt;-- CYBER INTELLIGENCE SYSTEM v1.0 --&gt;
          </div>
          <div className="inline-block border border-green-500 p-2 animate-pulse">
            [ TOP SECRET - AUTHORIZED ACCESS ONLY ]
          </div>
        </div>
        {sourcesLoading ? (
          <div className="mb-8 bg-black rounded-lg border border-green-500 p-6">
            <h2 className="text-xl font-bold text-green-400 flex items-center mb-4">
              &gt; TOP SOURCES_
            </h2>
            <div className="text-center">
              <div className="text-green-300 animate-pulse">LOADING SOURCE DATA...</div>
              <div className="text-xs mt-2 text-green-400">[////////////////////]</div>
            </div>
          </div>
        ) : sourceCounts.length > 0 && (
                <div className="mb-8 bg-black rounded-lg border border-green-500 p-6">
                  <h2 className="text-xl font-bold text-green-400 flex items-center mb-4">
                    {/* <svg xmlns="http://www.w3.org/2000/svg" className="h-6 w-6 mr-2" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M7 8h10M7 12h4m1 8l-4-4H5a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v8a2 2 0 01-2 2h-3l-4 4z" />
                    </svg> */}
                    &gt; TOP SOURCES_
                  </h2>
                  
                  <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-3">
                    {sourceCounts.map(([domain, count], index) => (
                      <div key={`domain-${index}`} className="bg-black p-2 border border-green-700 rounded">
                        <div className="text-green-400 font-mono text-sm truncate">
                          {domain}
                        </div>
                        <div className="text-green-300 font-mono text-xs">
                          [{count} stories]
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              )}
        {/* Latest data section */}
        {latestLoading ? (
          <div className="text-center p-8">
            <div className="text-green-300 animate-pulse">ACCESSING LATEST INTEL...</div>
            <div className="text-xs mt-2 text-green-400">[////////////////////]</div>
            <div className="flex justify-center mt-4">
              <ClockIcon size={48} fill="#00ff00" />
            </div>
          </div>
        ) : (
          kafkaData && (
            <>
              <div 
                className="mb-8 bg-black rounded-lg border border-green-500 p-6" 
                style={{ boxShadow: '0 0 10px #00ff00' }}
              >
                <div className="flex justify-between items-center mb-4">
                  <h2 className="text-xl font-bold text-green-400 flex items-center">
                    <ClockIcon size={24} className="mr-2" />
                    &gt; LATEST INTEL_
                  </h2>
                  <div className="text-xs text-green-300 animate-pulse flex items-center">
                    <ClockIcon size={16} className="mr-2" fill="#00ff00" />
                    [ LIVE FEED ]
                  </div>
                </div>

                <div 
                  className="bg-black p-4 rounded overflow-x-auto border border-green-700" 
                  style={{ 
                    backgroundImage: 'linear-gradient(rgba(0,50,0,0.1) 1px, transparent 1px)',
                    backgroundSize: '100% 2px' 
                  }}
                >
                  {JSON.parse(kafkaData).hour && (
                    <HourTimestamp timestamp={JSON.parse(kafkaData).hour} />
                  )}
                  
                  {JSON.parse(kafkaData).stories.map((story: StoryInfo, index: number) => (
                    <StoryCard key={`latest-${index}`} story={story} />
                  ))}
                </div>
              </div>
            
              {/* Source Counts Section */}
              
            </>
          )
        )}
        
        {/* Historical data section */}
        {!latestLoading && historyLoading ? (
          <div className="mt-8 text-center p-4 border border-green-900 rounded">
            <div className="text-green-300 flex items-center justify-center">
              <ClockIcon size={16} className="mr-2" fill="#00ff00" />
              <span className="animate-pulse">ACCESSING HISTORICAL ARCHIVES...</span>
            </div>
          </div>
        ) : (
          historicalData && historicalData.hours && historicalData.hours.length > 0 && (
            <div className="mt-8">
              <h2 className="text-xl font-bold text-green-400 flex items-center mb-4">
                <ClockIcon size={24} className="mr-2" />
                &gt; HISTORICAL ARCHIVES_
              </h2>
              
              {historicalData.hours.map((hourData, hourIndex) => (
                <div key={`hour-${hourIndex}`} className="mb-6 bg-black rounded-lg border border-green-500 p-4">
                  <HourTimestamp timestamp={hourData.hour} />
                  
                  <div 
                    className="bg-black p-3 rounded overflow-x-auto border border-green-700" 
                    style={{ 
                      backgroundImage: 'linear-gradient(rgba(0,50,0,0.1) 1px, transparent 1px)',
                      backgroundSize: '100% 2px' 
                    }}
                  >
                    {hourData.stories.slice(0, 10).map((story, storyIndex) => (
                      <StoryCard 
                        key={`hour-${hourIndex}-story-${storyIndex}`} 
                        story={story}
                      />
                    ))}
                  </div>
                </div>
              ))}
            </div>
          )
        )}

        {/* Footer */}
        <div className="text-center text-xs text-green-700 mt-8 flex flex-col items-center">
          <div className="flex justify-center space-x-2 mb-2">
            <ClockIcon size={24} fill="#00aa00" />
          </div>
          &lt;-- END OF TRANSMISSION --&gt;<br/>
          © 1997 CYBER COLLECTIVE // NO UNAUTHORIZED ACCESS
        </div>
      </div>
    </div>
  );
}

export default App;