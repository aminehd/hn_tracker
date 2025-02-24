/* eslint-disable @typescript-eslint/no-unused-vars */
import { useEffect, useState } from 'react';

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

function App() {
  const [updates, setUpdates] = useState<HourlyData[]>([]);
  const [nextUpdate, setNextUpdate] = useState<number>(0);
  const [loading, setLoading] = useState(true);

  const fetchLatestData = async () => {
    try {
      const response = await fetch('http://localhost:3000/api/latest');
      if (!response.ok) {
        throw new Error('Failed to fetch data');
      }
      const data = await response.json();
      setUpdates(prev => [data, ...prev.slice(0, 23)]); // Keep last 24 hours
      setLoading(false);
    } catch (error) {
      console.error('Error fetching data:', error);
      setLoading(false);
    }
  };

  useEffect(() => {
    // Fetch data immediately when component mounts
    fetchLatestData();

    // Calculate time until next hour
    const now = new Date();
    const nextHour = new Date(now.getFullYear(), now.getMonth(), now.getDate(), now.getHours() + 1, 0, 0);
    const msUntilNextHour = nextHour.getTime() - now.getTime();
    
    // Set initial countdown
    setNextUpdate(Math.floor(msUntilNextHour / 1000));

    // Update countdown every second
    const countdownInterval = setInterval(() => {
      setNextUpdate(prev => {
        if (prev <= 0) {
          fetchLatestData();
          return 3600; // Reset to 1 hour
        }
        return prev - 1;
      });
    }, 1000);

    return () => clearInterval(countdownInterval);
  }, []);

  const formatCountdown = (seconds: number): string => {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins}:${secs.toString().padStart(2, '0')}`;
  };

  if (loading) {
    return (
      <div className="container mx-auto p-4 text-center">
        <h1 className="text-2xl font-bold mb-4">Loading...</h1>
      </div>
    );
  }

  return (
    <div className="container mx-auto p-4">
      <div className="flex justify-between items-center mb-6">
        <h1 className="text-2xl font-bold">Hacker News Hourly Updates</h1>
        <div className="text-gray-600">
          Next update in: {formatCountdown(nextUpdate)}
        </div>
      </div>

      {updates.length === 0 ? (
        <div className="text-center text-gray-600">
          No updates available yet
        </div>
      ) : (
        updates.map((hourData) => (
          <div key={hourData.hour} className="mb-8 bg-white rounded-lg shadow p-6">
            <h2 className="text-xl font-bold mb-4">
              Update for {new Date(hourData.hour).toLocaleString()}
            </h2>
            
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mb-4">
              <div className="bg-gray-50 p-4 rounded">
                <h3 className="font-bold mb-2">Stats</h3>
                <p>Average Score: {hourData.avg_score.toFixed(1)}</p>
                <p>Total Comments: {hourData.total_comments}</p>
              </div>
              
              <div className="bg-gray-50 p-4 rounded">
                <h3 className="font-bold mb-2">Top Domains</h3>
                {hourData.domains.slice(0, 3).map(([domain, count]) => (
                  <p key={domain}>{domain}: {count} stories</p>
                ))}
              </div>
            </div>

            <h3 className="font-bold mb-2">Stories</h3>
            <div className="space-y-4">
              {hourData.stories.map((story, i) => (
                <div key={i} className="border-b pb-2">
                  <a 
                    href={story.url} 
                    target="_blank" 
                    rel="noopener noreferrer"
                    className="text-blue-600 hover:text-blue-800 font-medium"
                  >
                    {story.title}
                  </a>
                  <div className="text-sm text-gray-600">
                    by {story.author} | {story.score} points | {story.comments} comments
                    {story.domain && ` | ${story.domain}`}
                  </div>
                </div>
              ))}
            </div>
          </div>
        ))
      )}
    </div>
  );
}

export default App; 