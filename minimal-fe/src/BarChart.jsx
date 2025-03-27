import { useRef, useEffect } from 'react';
import * as d3 from 'd3';

const BarChart = ({ data }) => {
  const svgRef = useRef();

  useEffect(() => {
    if (!data || data.length === 0) return;

    // Clear any existing chart
    d3.select(svgRef.current).selectAll('*').remove();

    // Set up dimensions and margins
    const margin = { top: 30, right: 30, bottom: 70, left: 60 };
    const width = 800 - margin.left - margin.right;
    const height = 400 - margin.top - margin.bottom;

    // Create SVG element
    const svg = d3
      .select(svgRef.current)
      .attr('width', width + margin.left + margin.right)
      .attr('height', height + margin.top + margin.bottom)
      .append('g')
      .attr('transform', `translate(${margin.left},${margin.top})`);

    // X axis
    const x = d3
      .scaleBand()
      .range([0, width])
      .domain(data.map(d => d.domain))
      .padding(0.2);

    svg
      .append('g')
      .attr('transform', `translate(0,${height})`)
      .call(d3.axisBottom(x))
      .selectAll('text')
      .attr('transform', 'translate(-10,0)rotate(-45)')
      .style('text-anchor', 'end')
      .style('font-size', '12px');

    // Title for X axis
    svg
      .append('text')
      .attr('text-anchor', 'middle')
      .attr('x', width / 2)
      .attr('y', height + margin.bottom - 5)
      .text('Domains')
      .style('font-size', '14px');

    // Find the maximum count for Y axis
    const maxCount = d3.max(data, d => d.count);

    // Y axis
    const y = d3
      .scaleLinear()
      .domain([0, maxCount * 1.1]) // Add 10% padding
      .range([height, 0]);

    svg.append('g').call(d3.axisLeft(y));

    // Title for Y axis
    svg
      .append('text')
      .attr('text-anchor', 'middle')
      .attr('transform', 'rotate(-90)')
      .attr('y', -margin.left + 15)
      .attr('x', -height / 2)
      .text('Story Count')
      .style('font-size', '14px')
      .style('fill', '#ff6ec7'); // Neon pink color

    // Create bars with animation
    const bars = svg
      .selectAll('bars')
      .data(data)
      .enter()
      .append('rect')
      .attr('x', d => x(d.domain))
      .attr('width', x.bandwidth())
      .attr('y', height) // Start from bottom
      .attr('height', 0) // Start with height 0
      .attr('fill', (d, i) => (i === 0 ? '#66ff66' : '#33cc33'));
    
    // Animate bars growing from bottom
    bars.transition()
      .duration(1000)
      .delay((d, i) => i * 100)
      .attr('y', d => y(d.count))
      .attr('height', d => height - y(d.count))
      .on('end', startRandomAnimations);
    
    // Function to start random animations
    function startRandomAnimations() {
      // Add random pulse animations to bars
      bars.each(function(d, i) {
        const bar = d3.select(this);
        animateRandomly(bar, i);
      });
    }
    
    // Function to animate a bar randomly
    function animateRandomly(bar, index) {
      // Choose a random animation type
      const animType = Math.floor(Math.random() * 3);
      
      switch(animType) {
        case 0: // Pulse animation
          pulseAnimation(bar, index);
          break;
        case 1: // Glow animation
          glowAnimation(bar, index);
          break;
        case 2: // Bounce animation
          bounceAnimation(bar, index);
          break;
      }
      
      // Schedule next animation after random delay
      setTimeout(() => {
        animateRandomly(bar, index);
      }, Math.random() * 5000 + 2000); // Random delay between 2-7 seconds
    }
    
    // Pulse animation
    function pulseAnimation(bar, index) {
      const originalHeight = parseFloat(bar.attr('height'));
      const originalY = parseFloat(bar.attr('y'));
      
      bar.transition()
        .duration(300)
        .attr('height', originalHeight * 1.1)
        .attr('y', originalY - (originalHeight * 0.1))
        .transition()
        .duration(300)
        .attr('height', originalHeight)
        .attr('y', originalY);
    }
    
    // Glow animation
    function glowAnimation(bar, index) {
      const originalFill = index === 0 ? '#66ff66' : '#33cc33';
      
      bar.transition()
        .duration(400)
        .attr('fill', '#00ff00')
        .transition()
        .duration(400)
        .attr('fill', originalFill);
    }
    
    // Bounce animation
    function bounceAnimation(bar, index) {
      const originalY = parseFloat(bar.attr('y'));
      
      // Ensure the bar doesn't go below the bottom of the chart
      const minY = Math.min(height - parseFloat(bar.attr('height')), originalY);
      
      bar.transition()
        .duration(200)
        .attr('y', Math.max(originalY - 10, 0)) // Prevent going above the chart
        .transition()
        .duration(200)
        .attr('y', Math.max(minY, originalY)) // Ensure it doesn't go below the bottom
        .transition()
        .duration(100)
        .attr('y', Math.max(originalY - 5, 0))
        .transition()
        .duration(100)
        .attr('y', originalY);
    }

    // Add mouseover and mouseout events
    bars
      .on('mouseover', function(event, d) {
        d3.select(this).attr('fill', '#00ff00');
        
        // Add tooltip
        svg.append('text')
          .attr('class', 'tooltip')
          .attr('x', x(d.domain) + x.bandwidth() / 2)
          .attr('y', y(d.count) - 10)
          .attr('text-anchor', 'middle')
          .text(`${d.count}`)
          .style('font-size', '12px')
          .style('font-weight', 'bold')
          .style('fill', '#00bfff')
      })
      .on('mouseout', function() {
        const i = data.findIndex(item => item.domain === d3.select(this).data()[0].domain);
        d3.select(this).attr('fill', i === 0 ? '#66ff66' : '#33cc33');
        
        // Remove tooltip
        svg.selectAll('.tooltip').remove();
      });

    // Add chart title with animation
    const title = svg
      .append('text')
      .attr('x', width / 2)
      .attr('y', -10)
      .attr('text-anchor', 'middle')
      .style('font-size', '16px')
      .style('font-weight', 'bold')
      .style('fill', '#ff6ec7') // Neon pink color
      .style('opacity', 0)
      .text('Top Domains on Hacker News');
    
    // Animate title
    title.transition()
      .duration(1000)
      .style('opacity', 1);
    
    // Add subtle title animation
    setInterval(() => {
      title.transition()
        .duration(1500)
        .style('fill', '#ff6ec7')
        .transition()
        .duration(1500)
        .style('fill', '#ff9ed8');
    }, 3000);

  }, [data]);

  return (
    <div className="chart-container" style={{ marginTop: '20px', overflowX: 'auto' }}>
      <svg ref={svgRef}></svg>
    </div>
  );
};

export default BarChart;